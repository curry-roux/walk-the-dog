use anyhow::{anyhow, Result};

use web_sys::{AudioContext, AudioBuffer, AudioBufferSourceNode, AudioDestinationNode, AudioNode, GainNode};
use js_sys::ArrayBuffer;
use wasm_bindgen::{JsValue, JsCast};
use wasm_bindgen_futures::JsFuture;


pub fn create_audio_context() -> Result<AudioContext> {
    AudioContext::new().map_err(|err| anyhow!("Could not create audio context: {:#?}", err))
}

fn create_buffer_source(ctx: &AudioContext) -> Result<AudioBufferSourceNode> {
    ctx.create_buffer_source()
        .map_err(|err| anyhow!("Error creating buffer source: {:#?}", err))
}

fn connect_with_audio_node(
    buffer_source: &AudioBufferSourceNode,
    destination: &AudioDestinationNode,
) -> Result<AudioNode> {
    buffer_source
        .connect_with_audio_node(destination)
        .map_err(|err| anyhow!("Error connecting buffer source with destination: {:#?}", err))
}

pub fn play_sound(ctx: &AudioContext, buffer: &AudioBuffer, looping: LOOPING) -> Result<()> {
    let track_source = create_track_source(ctx, buffer)?;
    if matches!(looping, LOOPING::YES) {
        track_source.set_loop(true);
    }

    track_source
        .start()
        .map_err(|err| anyhow!("Could not start audio: {:#?}", err))
}

fn create_track_source(
    ctx: &AudioContext,
    buffer: &AudioBuffer)
-> Result<AudioBufferSourceNode> {
    let track_source = create_buffer_source(ctx)?;
    track_source.set_buffer(Some(&buffer));

    // 音量の調節をする
    let gain: GainNode = ctx.create_gain().map_err(|err| anyhow!("Error creating gain node: {:#?}", err))?;
    gain.gain().set_value(0.05);
    // 接続順序: track_source -> gain -> destination
    track_source.connect_with_audio_node(&gain).map_err(|err| anyhow!("Error connecting source with gain node: {:#?}", err))?;
    gain.connect_with_audio_node(&ctx.destination()).map_err(|err| anyhow!("Error connecting gain node with destination: {:#?}", err))?;

    //connect_with_audio_node(&track_source, &ctx.destination())?;
    Ok(track_source)
}

pub enum LOOPING {
    YES,
    NO,
}

pub async fn decode_audio_data(ctx: &AudioContext, array_buffer: &ArrayBuffer) -> Result<AudioBuffer> {
    JsFuture::from(
        ctx.decode_audio_data(&array_buffer)
            .map_err(|err| anyhow!("Error decoding audio data: {:#?}", err))?,
    )
    .await
    .map_err(|err| anyhow!("Could not convert promise to future {:#?}", err))?
    .dyn_into()
    .map_err(|err| anyhow!("Could not cast into AudioBuffer {:#?}", err))
}

