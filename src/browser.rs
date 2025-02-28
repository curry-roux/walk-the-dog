/* ブラウザとやりとりするためのコード */
use anyhow::{anyhow,Result};
use std::{future::Future};

use wasm_bindgen::{
    closure::WasmClosure, closure::WasmClosureFnOnce, prelude::Closure, JsCast, JsValue,
};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Window, Document, HtmlCanvasElement, CanvasRenderingContext2d, Response, HtmlImageElement, Element, HtmlElement};
use js_sys::ArrayBuffer;

macro_rules! log {
    ( $($t:tt)* ) => {
        web_sys::console::log_1(&format!( $($t)* ).into());
    }
}

pub fn window() -> Result<Window> {
    web_sys::window().ok_or_else(|| anyhow!("No window found."))
}

pub fn document() -> Result<Document> {
    window()?.document().ok_or_else(|| anyhow!("No document found."))
}

// canvasがハードコーディングされているが一旦このままで問題が起きたら修正する
pub fn canvas() -> Result<HtmlCanvasElement>{
    document()?
        .get_element_by_id("canvas")
        .ok_or_else(|| anyhow!("No canvas Element found with id 'canvas'"))?
        .dyn_into::<HtmlCanvasElement>()
        .map_err(|element| anyhow!("Error converting {:#?} to HtmlCanvasElement", element))
}

pub fn context() -> Result<CanvasRenderingContext2d>{
    canvas()?
        .get_context("2d")
        .map_err(|js_value| anyhow!("Error getting 2d context {:#?}", js_value))?
        .ok_or_else(|| anyhow!("2d context not found"))?
        .dyn_into::<CanvasRenderingContext2d>()
        .map_err(|element|{
            anyhow!("Error converting {:#?} to CanvasRenderingContext2d", element)
        })
}

pub fn spawn_local<F>(future: F)
where 
    F: Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(future);
}

pub async fn fetch_with_str(resource: &str) -> Result<JsValue>{
    JsFuture::from(window()?.fetch_with_str(resource))
        .await
        .map_err(|err| anyhow!("Failed to fetch resource {:#?}", err))
}

pub async fn fetch_json(json_path: &str) -> Result<JsValue>{
    let resp = fetch_responce(json_path).await?;

    JsFuture::from(
        resp.json().map_err(|err| anyhow!("Failed to call json() on Response {:#?}", err))?,
    )
    .await
    .map_err(|err| anyhow!("Failed to parse JSON {:#?}", err))
}

pub async fn fetch_array_buffer(resource: &str) -> Result<ArrayBuffer>{
    let array_buffer = fetch_responce(resource)
        .await?
        .array_buffer()
        .map_err(|err| anyhow!("Failed loading array buffer {:#?}", err))?;

    JsFuture::from(array_buffer)
        .await
        .map_err(|err| anyhow!("Error converting array buffer into a future {:#?}", err))?
        .dyn_into()
        .map_err(|err| anyhow!("Error converting raw JSValue ti ArrayBuffer {:#?}", err))
}

pub async fn fetch_responce(resource: &str) -> Result<Response>{
    fetch_with_str(resource)
        .await?
        .dyn_into()
        .map_err(|err| anyhow!("Error converting {:#?} to Response", err))
}

pub fn new_image() -> Result<HtmlImageElement>{
    HtmlImageElement::new()
        .map_err(|err| anyhow!("Could not create HtmlImageElement: {:#?}", err))
}

pub fn closure_once<F, A, R>(fn_once: F) -> Closure<F::FnMut>
where
    F: 'static + WasmClosureFnOnce<A, R>,
{
    Closure::once(fn_once)
}

pub type LoopClosure = Closure<dyn FnMut(f64)>;

pub fn request_animation_frame(callback: &LoopClosure) -> Result<i32>{
    window()?
        .request_animation_frame(callback.as_ref().unchecked_ref())
        .map_err(|err| anyhow!("Failed to request animation frame {:#?}", err))
}

pub fn create_ref_closure(f: impl FnMut(f64) + 'static) -> LoopClosure {
    closure_wrap(Box::new(f))
}

pub fn closure_wrap<T: WasmClosure + ?Sized>(data: Box<T>) -> Closure<T>{
    Closure::wrap(data)
}

pub fn now() -> Result<f64>{
    Ok(window()?
        .performance()
        .ok_or_else(|| anyhow!("Performance object not found"))?
        .now())
}

pub fn draw_ui(html: &str) -> Result<()>{
    find_ui()?
        .insert_adjacent_html("afterbegin", html)
        .map_err(|err| anyhow!("Could not insert html {:#?}", err))
}

pub fn hide_ui() -> Result<()>{
    let ui = find_ui()?;

    if let Some(child) = ui.first_child() {
        ui.remove_child(&child)
            .map(|_removed_child| ())
            .map_err(|err| anyhow!("Failed to remove child {:#?}", err))
            .and_then(|_unit| {
                canvas()?
                    .focus()
                    .map_err(|err| anyhow!("Could not set focus to canvas {:#?}", err))
            })
    } else {
        Ok(())
    }
}

fn find_ui() -> Result<Element> {
    document().and_then(|doc| {
        doc.get_element_by_id("ui")
            .ok_or_else(|| anyhow!("UI element not found"))
    })
}

pub fn find_html_element_by_id(id: &str) -> Result<HtmlElement> {
    document()
        .and_then(|doc| {
            doc.get_element_by_id(id)
                .ok_or_else(|| anyhow!("Element with id {} not found", id))
        })
        .and_then(|element| {
            element
                .dyn_into::<HtmlElement>()
                .map_err(|err| anyhow!("Could not cast into HtmlElement {:#?}", err))
        })
}