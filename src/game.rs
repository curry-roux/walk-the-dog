use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use rand::prelude::*;
use gloo_utils::format::JsValueSerdeExt;
use std::rc::Rc;
use futures::channel::mpsc::UnboundedReceiver;

use web_sys::HtmlImageElement;
use self::red_hat_boy_states::*;

use crate::{
    browser,
    engine::{
        self, Cell, Game, Image, KeyState, Point, Rect, Renderer, Sheet, SpriteSheet, Audio, Sound,
    }, 
    segments::{stone_and_platform, platform_and_stone},
};

const HEIGHT: i16 = 600;
const TIMELINE_MINIMUM: i16 = 1000;
const OBSTACLE_BUFFER:i16 = 20;


// pub enum WalkTheDog{
//     Loading,
//     Loaded(Walk),
// }

pub struct WalkTheDog{
    machine: Option<WalkTheDogStateMachine>,
}

enum WalkTheDogStateMachine{
    Ready(WalkTheDogState<Ready>),
    Walking(WalkTheDogState<Walking>),
    GameOver(WalkTheDogState<GameOver>),
}

impl WalkTheDogStateMachine {
    fn new(walk: Walk) -> Self {
        WalkTheDogStateMachine::Ready(WalkTheDogState::new(walk))
    }

    fn update(self, keystate: &KeyState) -> Self {
        match self {
            WalkTheDogStateMachine::Ready(state) => state.update(keystate).into(),
            WalkTheDogStateMachine::Walking(state) => state.update(keystate).into(),
            WalkTheDogStateMachine::GameOver(state) => state.update().into(),
        }
    }

    fn draw(&self, renderer: &Renderer) {
        match self {
            WalkTheDogStateMachine::Ready(state) => state.draw(renderer),
            WalkTheDogStateMachine::Walking(state) => state.draw(renderer),
            WalkTheDogStateMachine::GameOver(state) => state.draw(renderer),
        }
    }
}

struct WalkTheDogState<T> {
    _state: T,
    walk: Walk,
}

impl<T> WalkTheDogState<T> {
    fn draw(&self, renderer: &Renderer) {
        self.walk.draw(renderer);
    }
}

impl WalkTheDogState<Ready> {
    fn new(walk: Walk) -> WalkTheDogState<Ready> {
        WalkTheDogState {
            _state: Ready,
            walk,
        }
    }

    fn update(mut self, keystate: &KeyState) -> ReadyEndState {
        self.walk.boy.update();
        if keystate.is_pressed("ArrowRight") {
            ReadyEndState::Complete(self.start_running())
        } else {
            ReadyEndState::Continue(self)
        }
    }
}

impl WalkTheDogState<Ready> {
    fn start_running(mut self) -> WalkTheDogState<Walking> {
        self.run_right();
        WalkTheDogState {
            _state: Walking,
            walk: self.walk,
        }
    }

    fn run_right(&mut self) {
        self.walk.boy.run_right();
    }
}

impl WalkTheDogState<Walking> {
    fn update(mut self, keystate: &KeyState) -> WalkingEndState {
        if keystate.is_pressed("ArrowDown"){
            self.walk.boy.slide();
        }
        if keystate.is_pressed("Space"){
            self.walk.boy.jump();
        }
        self.walk.boy.update();

        let walk_speed = self.walk.velocity();
        let [first_background, second_background] = &mut self.walk.backgrounds;
        first_background.move_horizontally(walk_speed);
        second_background.move_horizontally(walk_speed);
        if first_background.right() < 0 {
            first_background.set_x(second_background.right());
        }
        if second_background.right() < 0 {
            second_background.set_x(first_background.right());
        }

        self.walk.obstacles.retain(|obstacle| obstacle.right() > 0);
        self.walk.obstacles.iter_mut().for_each(|obstacle| {
            obstacle.move_horizontally(walk_speed);
            obstacle.check_intersection(&mut self.walk.boy);
        });

        if self.walk.timeline < TIMELINE_MINIMUM {
            self.walk.generate_next_segment();
        } else {
            self.walk.timeline += walk_speed;
        }

        if self.walk.knocked_out() {
            WalkingEndState::Complete(self.end_game())
        } else {
            WalkingEndState::Continue(self)
        }
    }

    fn end_game(self) -> WalkTheDogState<GameOver> {
        let receiver = browser::draw_ui("<button id='new_game'>New Game</button>")
            .and_then(|_unit| browser::find_html_element_by_id("new_game"))
            .map(|element| engine::add_click_handler(element))
            .unwrap();

        WalkTheDogState {
            _state: GameOver{
                new_game_event: receiver,
            },
            walk: self.walk,
        }
    }
}

impl WalkTheDogState<GameOver> {
    fn update(mut self) -> GameOverEndState {
        if self._state.new_game_pressed() {
            GameOverEndState::Complete(self.new_game())
        } else {
            GameOverEndState::Continue(self)
        }
    }

    fn new_game(self) -> WalkTheDogState<Ready> {
        browser::hide_ui();
        WalkTheDogState {
            _state: Ready,
            walk: Walk::reset(self.walk),
        }
    }
}

impl From<WalkTheDogState<Ready>> for WalkTheDogStateMachine {
    fn from(state: WalkTheDogState<Ready>) -> Self {
        WalkTheDogStateMachine::Ready(state)
    }
}

impl From<WalkTheDogState<Walking>> for WalkTheDogStateMachine {
    fn from(state: WalkTheDogState<Walking>) -> Self {
        WalkTheDogStateMachine::Walking(state)
    }
}

impl From<WalkTheDogState<GameOver>> for WalkTheDogStateMachine {
    fn from(state: WalkTheDogState<GameOver>) -> Self {
        WalkTheDogStateMachine::GameOver(state)
    }
}

impl From<WalkingEndState> for WalkTheDogStateMachine {
    fn from(end_state: WalkingEndState) -> Self {
        match end_state {
            WalkingEndState::Complete(game_over) => game_over.into(),
            WalkingEndState::Continue(walking) => walking.into(),
        }
    }
}

impl From<GameOverEndState> for WalkTheDogStateMachine {
    fn from(end_state: GameOverEndState) -> Self {
        match end_state {
            GameOverEndState::Complete(ready) => ready.into(),
            GameOverEndState::Continue(game_over) => game_over.into(),
        }
    }
}


struct Ready;
struct Walking;
struct GameOver {
    new_game_event: UnboundedReceiver<()>,
}

impl GameOver {
    fn new_game_pressed(&mut self) -> bool {
        matches!(self.new_game_event.try_next(), Ok(Some(())))
    }
}

enum ReadyEndState {
    Complete(WalkTheDogState<Walking>),
    Continue(WalkTheDogState<Ready>),
}
impl From<ReadyEndState> for WalkTheDogStateMachine {
    fn from(state: ReadyEndState) -> Self {
        match state {
            ReadyEndState::Complete(walking) => walking.into(),
            ReadyEndState::Continue(ready) => ready.into(),
        }
    }
}

enum WalkingEndState {
    Complete(WalkTheDogState<GameOver>),
    Continue(WalkTheDogState<Walking>),
}

enum GameOverEndState {
    Complete(WalkTheDogState<Ready>),
    Continue(WalkTheDogState<GameOver>),
}



pub trait Obstacle {
    fn check_intersection(&self, boy: &mut RedHatBoy);
    fn draw(&self, renderer: &Renderer);
    fn draw_rect(&self, renderer: &Renderer);
    fn move_horizontally(&mut self, distance: i16);
    fn right(&self) -> i16;
}

pub struct Walk{
    boy: RedHatBoy,
    backgrounds: [Image; 2],
    obstacles: Vec<Box<dyn Obstacle>>,
    obstacle_sheet: Rc<SpriteSheet>,
    stone: HtmlImageElement,
    timeline: i16,
}

impl Walk{
    fn velocity(&self) -> i16{
        -self.boy.walking_speed()
    }

    fn generate_next_segment(&mut self){
        let mut rng = rand::thread_rng();
        let next_segment = rng.gen_range(0..2);

        let mut next_obstacles = match next_segment {
            0 => stone_and_platform(
                self.stone.clone(),
                self.obstacle_sheet.clone(),
                self.timeline + OBSTACLE_BUFFER,
            ),
            1 => platform_and_stone(
                self.stone.clone(),
                self.obstacle_sheet.clone(),
                self.timeline + OBSTACLE_BUFFER,
            ),
            _ => vec![],
        };

        self.timeline = rightmost(&next_obstacles);
        self.obstacles.append(&mut next_obstacles);
    }

    fn knocked_out(&self) -> bool {
        self.boy.knocked_out()
    }

    fn reset(walk: Self) -> Self {
        let starting_obstacles = stone_and_platform(walk.stone.clone(), walk.obstacle_sheet.clone(), 0);
        let timeline = rightmost(&starting_obstacles);

        Walk {
            boy: RedHatBoy::reset(walk.boy),
            backgrounds: walk.backgrounds,
            obstacles: starting_obstacles,
            obstacle_sheet: walk.obstacle_sheet,
            stone: walk.stone,
            timeline,
        }
    }

    fn draw(&self, renderer: &Renderer){
        self.backgrounds.iter().for_each(|background| {
            background.draw(renderer);
        });
        self.boy.draw(renderer);

        self.obstacles.iter().for_each(|obstacle| {
            obstacle.draw(renderer);
        });
    }
}

impl WalkTheDog{
    // pub fn new() -> Self{
    //     WalkTheDog::Loading
    // }
    pub fn new() -> Self {
        WalkTheDog {
            machine: None,
        }
    }
}

const LOW_PLATFORM: i16 = 420;
const FIRST_PLATFORM: i16 = 375;
#[async_trait(?Send)]
impl Game for WalkTheDog{
    async fn initialize(&self) -> Result<Box<dyn Game>>{
        match self.machine {
            None => {
                let sheet = browser::fetch_json("rhb.json").await?;
                let background = engine::load_image("BG.png").await?;
                let stone = engine::load_image("Stone.png").await?;

                let background_width = background.width() as i16;

                let tiles = browser::fetch_json("tiles.json").await?;
                let sprite_sheet = Rc::new(SpriteSheet::new(
                    tiles.into_serde::<Sheet>()?,
                    engine::load_image("tiles.png").await?,
                ));

                let starting_obstacles = stone_and_platform(stone.clone(), sprite_sheet.clone(), 0);
                let timeline = rightmost(&starting_obstacles);

                let platform = Platform::new(
                    sprite_sheet.clone(),
                    Point {x: FIRST_PLATFORM, y: LOW_PLATFORM},
                    &["13.png", "14.png", "15.png"],
                    &[
                        Rect::new_from_x_y(0, 0, 60, 54),
                        Rect::new_from_x_y(60, 0, 384 - (60*2), 93),
                        Rect::new_from_x_y(384 - 60, 0, 60, 54),
                    ]
                );

                let audio = Audio::new()?;
                let sound = audio.load_sound("SFX_Jump_23.mp3").await?;
                let background_music = audio.load_sound("background_song.mp3").await?;
                audio.play_looping_sound(&background_music)?;

                let rhb = RedHatBoy::new(
                    sheet.into_serde::<Sheet>()?,
                    engine::load_image("rhb.png").await?,
                    audio,
                    sound,
                );
                // Ok(Box::new(WalkTheDog::Loaded(Walk {
                //     boy: rhb,
                //     //background: Image::new(background, Point {x:0 , y:0}),
                //     backgrounds: [
                //         Image::new(background.clone(), Point {x:0 , y:0}),
                //         Image::new(background, Point {x:background_width , y:0}),
                //     ],
                //     obstacles: vec![
                //         Box::new(Barrier::new(Image::new(stone.clone(), Point {x:150, y:546}))),
                //         Box::new(platform),
                //     ],
                //     obstacle_sheet: sprite_sheet,
                //     stone,
                //     timeline,
                // })))
                let machine = WalkTheDogStateMachine::new(Walk {
                    boy: rhb,
                    backgrounds: [
                        Image::new(background.clone(), Point {x:0 , y:0}),
                        Image::new(background, Point {x:background_width , y:0}),
                    ],
                    obstacles: starting_obstacles,
                    obstacle_sheet: sprite_sheet,
                    stone,
                    timeline,
                    }
                );
                Ok(Box::new(WalkTheDog {
                    machine: Some(machine),
                }))
            }
            Some(_) => Err(anyhow!("Error: Game is already initialized!")),
        }
    }

    fn update(&mut self, keystate: &KeyState){
        //if let WalkTheDog::Loaded(walk) = self{
        if let Some(machine) = self.machine.take(){
            self.machine.replace(machine.update(keystate));

            // if keystate.is_pressed("ArrowRight"){
            //     walk.boy.run_right();
            // }
            // if keystate.is_pressed("ArrowDown"){
            //     walk.boy.slide();
            // }
            // if keystate.is_pressed("Space"){
            //     walk.boy.jump();
            // }
            // walk.boy.update();

            // let velocity = walk.velocity();
            // let [first_background, second_background] = &mut walk.backgrounds;
            // first_background.move_horizontally(velocity);
            // second_background.move_horizontally(velocity);
            // if first_background.right() < 0 {
            //     first_background.set_x(second_background.right());
            // }
            // if second_background.right() < 0 {
            //     second_background.set_x(first_background.right());
            // }

            // walk.obstacles.retain(|obstacle| obstacle.right() > 0);
            // walk.obstacles.iter_mut().for_each(|obstacle| {
            //     obstacle.move_horizontally(velocity);
            //     obstacle.check_intersection(&mut walk.boy);
            // });

            // if walk.timeline < TIMELINE_MINIMUM {
            //     walk.generate_next_segment();
            // } else {
            //     walk.timeline += velocity;
            // }
        }
        assert!(self.machine.is_some());
    }

    fn draw(&self, renderer: &Renderer){
        renderer.clear(&Rect::new_from_x_y(0, 0, 600, 600));

        if let Some(machine) = &self.machine {
            machine.draw(renderer);
        }

        // if let WalkTheDog::Loaded(walk) = self{
        //     //walk.background.draw(renderer);
        //     walk.backgrounds.iter().for_each(|background|{
        //         background.draw(renderer);
        //     });
        //     walk.boy.draw(renderer);
        //     walk.obstacles.iter().for_each(|obstacle| {
        //         obstacle.draw(renderer);
        //     });

        //     // 矩形を描画
        //     walk.boy.draw_rect(renderer);
        //     walk.obstacles.iter().for_each(|obstacle| {
        //         obstacle.draw_rect(renderer);
        //     });
        // }
    }
}

pub struct Barrier{
    image: Image,
}

impl Barrier {
    pub fn new(image: Image) -> Self {
        Barrier { image }
    }
}

impl Obstacle for Barrier {
    fn check_intersection(&self, boy: &mut RedHatBoy) {
        if boy.bounding_box().intersects(self.image.bounding_box()) {
            boy.knock_out();
        }
    }

    fn draw(&self, renderer: &Renderer) {
        self.image.draw(renderer);
    }

    fn draw_rect(&self, renderer: &Renderer) {
        self.image.draw_rect(renderer);
    }

    fn move_horizontally(&mut self, x: i16) {
        self.image.move_horizontally(x);
    }

    fn right(&self) -> i16 {
        self.image.right()
    }
}


pub struct Platform {
    sheet: Rc<SpriteSheet>,
    bounding_boxes: Vec<Rect>,
    sprites: Vec<Cell>,
    position: Point,
}

impl Platform {
    pub fn new(
        sheet: Rc<SpriteSheet>,
        position: Point,
        sprite_names: &[&str],
        bounding_boxes: &[Rect],
    ) -> Self {
        let sprites = sprite_names
            .iter()
            .filter_map(|sprite_name| sheet.cell(sprite_name).cloned())
            .collect();
    
        let bounding_boxes = bounding_boxes
            .iter()
            .map(|bounding_box| {
                Rect::new_from_x_y(
                    bounding_box.x() + position.x,
                    bounding_box.y() + position.y,
                    bounding_box.width,
                    bounding_box.height,
                )
            })
            .collect();

        Platform {
            sheet,
            position,
            sprites,
            bounding_boxes,
        }
    }

    fn bounding_boxes(&self) -> &Vec<Rect> {
        &self.bounding_boxes
    }

    fn destination_box(&self) -> Rect {
        let platform = self
            .sheet
            .cell("13.png")
            .expect("13.png does not exist");

        Rect::new_from_x_y(
            self.position.x.into(),
            self.position.y.into(),
            (platform.frame.w * 3) as i16,
            platform.frame.h as i16,
        )
    }
}

impl Obstacle for Platform{
    fn draw(&self, renderer: &Renderer) {
        let mut x = 0;
        self.sprites.iter().for_each(|sprite| {
            self.sheet.draw(
                renderer,
                &Rect::new_from_x_y(
                    sprite.frame.x,
                    sprite.frame.y,
                    sprite.frame.w,
                    sprite.frame.h,
                ),
                &Rect::new_from_x_y(
                    self.position.x + x,
                    self.position.y,
                    sprite.frame.w,
                    sprite.frame.h,
                ),
            );
            x += sprite.frame.w;
        });
    }

    fn draw_rect(&self, renderer: &Renderer) {
        for bounding_box in self.bounding_boxes() {
            renderer.draw_rect(&bounding_box);
        }
    }

    fn check_intersection(&self, boy: &mut RedHatBoy) {
        if let Some(box_to_land_on) = self
            .bounding_boxes()
            .iter()
            .find(|bounding_box| boy.bounding_box().intersects(bounding_box))
        {
            if boy.velocity_y() > 0 && boy.pos_y() < self.position.y {
                boy.land_on(box_to_land_on.y());
            } else {
                boy.knock_out();
            }
        }
    }
    
    fn move_horizontally(&mut self, x: i16) {
        self.position.x += x;
        self.bounding_boxes.iter_mut().for_each(|bounding_box| {
            bounding_box.set_x(bounding_box.x() + x);
        });
    }

    fn right(&self) -> i16 {
        self.bounding_boxes()
            .last()
            .unwrap_or(&Rect::default())
            .right()
    }
}

fn rightmost(obstacle_list: &Vec<Box<dyn Obstacle>>) -> i16 {
    obstacle_list
        .iter()
        .map(|obstacle| obstacle.right())
        .max_by(|x,y| x.cmp(y))
        .unwrap_or(0)
}


struct RedHatBoy{
    state_machine: RedHatBoyStateMachine,
    sprite_sheet: Sheet,
    image: HtmlImageElement,
}

impl RedHatBoy{
    fn new(sheet: Sheet, image: HtmlImageElement, audio: Audio, sound: Sound) -> Self{
        RedHatBoy{
            state_machine: RedHatBoyStateMachine::Idle(RedHatBoyState::new(audio, sound)),
            sprite_sheet: sheet,
            image,
        }
    }

    fn draw(&self, renderer: &Renderer){
        let sprite = self.current_sprite().expect("Cell not found");

        renderer.draw_image(
            &self.image,
            &Rect::new_from_x_y(
                sprite.frame.x as i16,
                sprite.frame.y as i16,
                sprite.frame.w as i16,
                sprite.frame.h as i16,
            ),
            &Rect::new_from_x_y(
                (self.state_machine.context().position.x + sprite.sprite_source_size.x as i16).into(),
                (self.state_machine.context().position.y + sprite.sprite_source_size.y as i16).into(),
                sprite.frame.w as i16,
                sprite.frame.h as i16,
            ),
        );
    }

    fn draw_rect(&self, renderer: &Renderer){
        renderer.draw_rect(&self.bounding_box());
    }

    fn bounding_box(&self) -> Rect {
        const X_OFFSET: i16 = 18;
        const Y_OFFSET: i16 = 14;
        const WIDTH_OFFSET: i16 = 28;
        Rect::new_from_x_y(
            self.destination_box().x() + X_OFFSET,
            self.destination_box().y() + Y_OFFSET,
            self.destination_box().width - WIDTH_OFFSET,
            self.destination_box().height - Y_OFFSET,
        )
    }

    fn destination_box(&self) -> Rect {
        let sprite = self.current_sprite().expect("Cell not found");

        Rect::new_from_x_y(
            (self.state_machine.context().position.x + sprite.sprite_source_size.x as i16).into(),
            (self.state_machine.context().position.y + sprite.sprite_source_size.y as i16).into(),
            sprite.frame.w as i16,
            sprite.frame.h as i16,
        )
    }

    fn frame_name(&self) -> String{
        format!(
            "{} ({}).png",
            self.state_machine.frame_name(),
            (self.state_machine.context().frame / 3) + 1,
        )
    }

    fn reset(boy: Self) -> Self{
        RedHatBoy::new(
            boy.sprite_sheet,
            boy.image,
            boy.state_machine.context().audio.clone(),
            boy.state_machine.context().jump_sound.clone(),
        )
    }

    fn knocked_out(&self) -> bool{
        self.state_machine.knocked_out()
    }

    fn pos_y(&self) -> i16{
        self.state_machine.context().position.y
    }

    fn velocity_y(&self) -> i16{
        self.state_machine.context().velocity.y
    }

    fn walking_speed(&self) -> i16{
        self.state_machine.context().velocity.x 
    }

    fn current_sprite(&self) -> Option<&Cell>{
        self.sprite_sheet.frames.get(&self.frame_name())
    }

    fn update(&mut self){
        self.state_machine = self.state_machine.clone().update();
    }

    fn run_right(&mut self){
        self.state_machine = self.state_machine.clone().transition(Event::Run);
    }

    fn slide(&mut self){
        self.state_machine = self.state_machine.clone().transition(Event::Slide);
    }

    fn jump(&mut self){
        self.state_machine = self.state_machine.clone().transition(Event::Jump);
    }

    fn land_on(&mut self, position: i16){
        self.state_machine = self.state_machine.clone().transition(Event::Land(position));
    }

    fn knock_out(&mut self){
        self.state_machine = self.state_machine.clone().transition(Event::KnockOut);
    }
}

#[derive(Clone)]
enum RedHatBoyStateMachine{
    Idle(RedHatBoyState<Idle>),
    Running(RedHatBoyState<Running>),
    Sliding(RedHatBoyState<Sliding>),
    Jumping(RedHatBoyState<Jumping>),
    Falling(RedHatBoyState<Falling>),
    KnockedOut(RedHatBoyState<KnockedOut>),
}
impl From<RedHatBoyState<Sliding>> for RedHatBoyStateMachine{
    fn from(state: RedHatBoyState<Sliding>) -> Self{
        RedHatBoyStateMachine::Sliding(state)
    }
}

#[derive(Clone)]
pub enum Event{
    Run,
    Slide,
    Update,
    Jump,
    KnockOut,
    Land(i16),
}

impl RedHatBoyStateMachine{
    fn transition(self, event: Event) -> Self{
        match(self.clone(), event){
            (RedHatBoyStateMachine::Idle(state), Event::Run) => state.run().into(),
            (RedHatBoyStateMachine::Running(state), Event::Slide) => state.slide().into(),
            (RedHatBoyStateMachine::Idle(state), Event::Update) => state.update().into(),
            (RedHatBoyStateMachine::Running(state), Event::Update) => state.update().into(),
            (RedHatBoyStateMachine::Sliding(state), Event::Update) => state.update().into(),
            (RedHatBoyStateMachine::Running(state), Event::Jump) => state.jump().into(),
            (RedHatBoyStateMachine::Jumping(state), Event::Update) => state.update().into(),
            (RedHatBoyStateMachine::Running(state), Event::KnockOut) => state.knock_out().into(),
            (RedHatBoyStateMachine::Sliding(state), Event::KnockOut) => state.knock_out().into(),
            (RedHatBoyStateMachine::Jumping(state), Event::KnockOut) => state.knock_out().into(),
            (RedHatBoyStateMachine::Falling(state), Event::Update) => state.update().into(),
            (RedHatBoyStateMachine::Jumping(state), Event::Land(position)) => state.land_on(position).into(),
            (RedHatBoyStateMachine::Running(state), Event::Land(position)) => state.land_on(position).into(),
            (RedHatBoyStateMachine::Sliding(state), Event::Land(position)) => state.land_on(position).into(),
            _ => self,
        }
    }

    fn frame_name(&self) -> &str{
        match self{
            RedHatBoyStateMachine::Idle(state) => state.frame_name(),
            RedHatBoyStateMachine::Running(state) => state.frame_name(),
            RedHatBoyStateMachine::Sliding(state) => state.frame_name(),
            RedHatBoyStateMachine::Jumping(state) => state.frame_name(),
            RedHatBoyStateMachine::Falling(state) => state.frame_name(),
            RedHatBoyStateMachine::KnockedOut(state) => state.frame_name(),
        }
    }

    fn context(&self) -> &RedHatBoyContext{
        match self{
            RedHatBoyStateMachine::Idle(state) => &state.context(),
            RedHatBoyStateMachine::Running(state) => &state.context(),
            RedHatBoyStateMachine::Sliding(state) => &state.context(),
            RedHatBoyStateMachine::Jumping(state) => &state.context(),
            RedHatBoyStateMachine::Falling(state) => &state.context(),
            RedHatBoyStateMachine::KnockedOut(state) => &state.context(),
        }
    }

    fn knocked_out(&self) -> bool {
        matches!(self, RedHatBoyStateMachine::KnockedOut(_))
    }

    fn update(self) -> Self{
        self.transition(Event::Update)
    }
}

impl From<RedHatBoyState<Running>> for RedHatBoyStateMachine{
    fn from(state: RedHatBoyState<Running>) -> Self{
        RedHatBoyStateMachine::Running(state)
    }
}

impl From<RedHatBoyState<Idle>> for RedHatBoyStateMachine{
    fn from(state: RedHatBoyState<Idle>) -> Self{
        RedHatBoyStateMachine::Idle(state)
    }
}

impl From<RedHatBoyState<Jumping>> for RedHatBoyStateMachine{
    fn from(state: RedHatBoyState<Jumping>) -> Self{
        RedHatBoyStateMachine::Jumping(state)
    }
}

impl From<SlidingEndState> for RedHatBoyStateMachine{
    fn from(end_state: SlidingEndState) -> Self{
        match end_state{
            SlidingEndState::Complete(running_state) => running_state.into(),
            SlidingEndState::Sliding(sliding_state) => sliding_state.into(),
        }
    }
}

impl From<JumpingEndState> for RedHatBoyStateMachine{
    fn from(end_state: JumpingEndState) -> Self{
        match end_state{
            JumpingEndState::Complete(running_state) => running_state.into(),
            JumpingEndState::Jumping(jumping_state) => jumping_state.into(),
        }
    }
}

impl From<RedHatBoyState<Falling>> for RedHatBoyStateMachine{
    fn from(state: RedHatBoyState<Falling>) -> Self{
        RedHatBoyStateMachine::Falling(state)
    }
}

impl From<FallingEndState> for RedHatBoyStateMachine{
    fn from(end_state: FallingEndState) -> Self{
        match end_state{
            FallingEndState::KnockedOut(knocked_out_state) => knocked_out_state.into(),
            FallingEndState::Falling(falling_state) => falling_state.into(),
        }
    }
}

impl From<RedHatBoyState<KnockedOut>> for RedHatBoyStateMachine{
    fn from(state: RedHatBoyState<KnockedOut>) -> Self{
        RedHatBoyStateMachine::KnockedOut(state)
    }
}

mod red_hat_boy_states{
    use crate::engine::Point;
    use super::{HEIGHT, Audio, Sound};

    const FLOOR: i16 = 479;
    const PLAYER_HEIGHT: i16 = HEIGHT - FLOOR;
    const GRAVITY: i16 = 1;
    const TERMINAL_VELOCITY: i16 = 20;
    const STARTING_POINT: i16 = -20;
    const RUNNING_SPEED: i16 = 4;
    const JUMP_SPEED: i16 = -25;

    const IDLE_FRAMES: u8 = 29;
    const RUNNING_FRAMES: u8 = 23;
    const SLIDING_FRAMES: u8 = 14;
    const JUMPING_FRAMES: u8 = 35;
    const FALLING_FRAMES: u8 = 29;
    const IDLE_FRAME_NAME: &str = "Idle";
    const RUN_FRAME_NAME: &str = "Run";
    const SLIDE_FRAME_NAME: &str = "Slide";
    const JUMP_FRAME_NAME: &str = "Jump";
    const FALL_FRAME_NAME: &str = "Dead";

    #[derive(Clone)]
    pub struct RedHatBoyState<S>{
        context: RedHatBoyContext,
        _state: S,
    }

    #[derive(Clone)]
    pub struct RedHatBoyContext{
        pub frame: u8,
        pub position: Point,
        pub velocity: Point,
        pub audio: Audio,
        pub jump_sound: Sound,
    }

    impl RedHatBoyContext{
        pub fn update(mut self, frame_count: u8) -> Self{
            if self.velocity.y < TERMINAL_VELOCITY{
                self.velocity.y += GRAVITY;
            }

            if self.frame < frame_count{
                self.frame += 1;
            } else {
                self.frame = 0;
            }

            //self.position.x += self.velocity.x;
            self.position.y += self.velocity.y;
            if self.position.y > FLOOR{
                self.position.y = FLOOR;
            }

            self
        }

        fn set_vertical_velocity(mut self, y: i16) -> Self{
            self.velocity.y = y;
            self
        }

        fn reset_frame(mut self) -> Self{
            self.frame = 0;
            self
        }

        fn run_right(mut self) -> Self{
            self.velocity.x = RUNNING_SPEED;
            self
        }

        fn stop(mut self) -> Self{
            self.velocity.x = 0;
            self
        }

        fn set_on(mut self, position: i16) -> Self{
            let position = position - PLAYER_HEIGHT;
            self.position.y = position;
            self
        }

        fn play_jump_sound(self) -> Self{
            if let Err(err) = self.audio.play_sound(&self.jump_sound) {
                log!("Error playing jump sound: {:#?}", err);
            }
            self
        }
    }

    #[derive(Debug, Copy, Clone)]
    pub struct Idle;

    #[derive(Debug, Copy, Clone)]
    pub struct Running;

    #[derive(Debug, Copy, Clone)]
    pub struct Sliding;

    #[derive(Debug, Copy, Clone)]
    pub struct Jumping;

    #[derive(Debug, Copy, Clone)]
    pub struct Falling;

    #[derive(Debug, Copy, Clone)]
    pub struct KnockedOut;

    impl<S> RedHatBoyState<S>{
        pub fn context(&self) -> &RedHatBoyContext{
            &self.context
        }
    }

    impl RedHatBoyState<Idle>{
        pub fn new(audio: Audio, jump_sound: Sound) -> Self{
            RedHatBoyState{
                context: RedHatBoyContext{
                    frame: 0,
                    position: Point{x: STARTING_POINT, y: FLOOR},
                    velocity: Point{x: 0, y: 0},
                    audio,
                    jump_sound,
                },
                _state: Idle {},
            }
        }

        pub fn run(self) -> RedHatBoyState<Running>{
            RedHatBoyState{
                context: self.context.reset_frame().run_right(),
                _state: Running {},
            }
        }

        pub fn update(mut self) -> Self{
            self.context = self.context.update(IDLE_FRAMES);
            self
        }

        pub fn frame_name(&self) -> &str{
            IDLE_FRAME_NAME
        }
    }

    impl RedHatBoyState<Running>{
        pub fn frame_name(&self) -> &str{
            RUN_FRAME_NAME
        }

        pub fn update(mut self) -> Self{
            self.context = self.context.update(RUNNING_FRAMES);
            self
        }

        pub fn slide(self) -> RedHatBoyState<Sliding>{
            RedHatBoyState{
                context: self.context.reset_frame(),
                _state: Sliding {},
            }
        }

        pub fn jump(self) -> RedHatBoyState<Jumping>{
            RedHatBoyState{
                context: self
                    .context
                    .set_vertical_velocity(JUMP_SPEED)
                    .reset_frame()
                    .play_jump_sound(),
                _state: Jumping {},
            }
        }

        pub fn knock_out(self) -> RedHatBoyState<Falling>{
            RedHatBoyState{
                context: self.context.reset_frame().stop(),
                _state: Falling {},
            }
        }

        pub fn land_on(self, position: i16) -> RedHatBoyState<Running>{
            RedHatBoyState{
                context: self.context.set_on(position),
                _state: Running {},
            }
        }
    }

    impl RedHatBoyState<Sliding>{
        pub fn frame_name(&self) -> &str{
            SLIDE_FRAME_NAME
        }

        pub fn update(mut self) -> SlidingEndState{
            self.context = self.context.update(SLIDING_FRAMES);
            if self.context.frame >= SLIDING_FRAMES{
                SlidingEndState::Complete(self.stand())
            } else {
                SlidingEndState::Sliding(self)
            }
        }

        pub fn stand(self) -> RedHatBoyState<Running>{
            RedHatBoyState{
                context: self.context.reset_frame(),
                _state: Running {},
            }
        }

        pub fn knock_out(self) -> RedHatBoyState<Falling>{
            RedHatBoyState{
                context: self.context.reset_frame().stop(),
                _state: Falling {},
            }
        }

        pub fn land_on(self, position: i16) -> RedHatBoyState<Sliding>{
            RedHatBoyState{
                context: self.context.set_on(position),
                _state: Sliding {},
            }
        }
    }

    impl RedHatBoyState<Jumping>{
        pub fn frame_name(&self) -> &str{
            JUMP_FRAME_NAME
        }

        pub fn update(mut self) -> JumpingEndState {
            self.context = self.context.update(JUMPING_FRAMES);
            if self.context.position.y >= FLOOR{
                JumpingEndState::Complete(self.land_on(HEIGHT.into()))
            } else {
                JumpingEndState::Jumping(self)
            }
        }

        pub fn land(self) -> RedHatBoyState<Running>{
            RedHatBoyState{
                context: self.context.reset_frame(),
                _state: Running {},
            }
        }

        pub fn land_on(self, position: i16) -> RedHatBoyState<Running>{
            log!("Landed on {}", position);
            RedHatBoyState{
                context: self.context.reset_frame().set_on(position),
                _state: Running,
            }
        }

        pub fn knock_out(self) -> RedHatBoyState<Falling>{
            RedHatBoyState{
                context: self.context.reset_frame().stop(),
                _state: Falling {},
            }
        }
    }

    impl RedHatBoyState<Falling>{
        pub fn frame_name(&self) -> &str{
            FALL_FRAME_NAME
        }

        pub fn update(mut self) -> FallingEndState{
            self.context = self.context.update(FALLING_FRAMES);
            if self.context.frame >= FALLING_FRAMES{
                FallingEndState::KnockedOut(self.knock_out())
            } else {
                FallingEndState::Falling(self)
            }
        }

        pub fn knock_out(self) -> RedHatBoyState<KnockedOut>{
            RedHatBoyState{
                context: self.context,
                _state: KnockedOut {},
            }
        }
    }

    impl RedHatBoyState<KnockedOut>{
        pub fn frame_name(&self) -> &str{
            FALL_FRAME_NAME
        }
    }

    pub enum SlidingEndState{
        Complete(RedHatBoyState<Running>),
        Sliding(RedHatBoyState<Sliding>),
    }

    pub enum JumpingEndState{
        Complete(RedHatBoyState<Running>),
        Jumping(RedHatBoyState<Jumping>),
    }

    pub enum FallingEndState{
        KnockedOut(RedHatBoyState<KnockedOut>),
        Falling(RedHatBoyState<Falling>),
    }
}