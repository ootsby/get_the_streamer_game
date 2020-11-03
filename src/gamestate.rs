use crate::{
    chat_test_mock,
    chatter::Chatter,
    command,
    command::CommandInstance,
    command::CommandParser,
    credits::Credits,
    draw_system::{DrawSystem, PlayerDrawSystem},
    game_object::GameObject,
    game_object_type::GameObjectType,
    interface::Interface,
    life_system::PlayerLifeSystem,
    physics::PlayerPhysics,
    running_state::RunningState,
    sprites::Sprite,
    utilities, RunConfig,
};

use ggez::audio;
use ggez::audio::SoundSource;
use ggez::event::EventHandler;
use ggez::graphics::BLACK;
use ggez::{graphics, timer, Context, GameResult};
use std::{
    collections::HashMap,
    sync::mpsc::{channel, Receiver, Sender},
    thread,
    time::Duration,
    time::Instant,
};
use twitch_chat_wrapper::chat_message::ChatMessage;

const LIVES: u8 = 3;
pub const FRAMERATE_TARGET: u32 = 60;
const GAME_TIME: Duration = Duration::from_secs(120);
const SPLASH_DURATION: Duration = Duration::from_secs(15);
const SCORES_FILE_NAME: &str = "/high_scores";

struct GameArea {
    pixel_width: f32,
    pixel_height: f32,
}

pub struct GameState {
    send_to_chat: Sender<String>,
    receive_from_chat: Receiver<ChatMessage>,
    screen_size: (f32, f32),
    interface: Interface,
    game_objects: Vec<GameObject>,
    player_hit_object_event: Receiver<Chatter>,
    running_state: RunningState,
    credits: Option<Credits>,
    game_start_time: Instant,
    object_sound: audio::Source,
    scores: HashMap<String, u128>,
    command_parser: CommandParser,
}

impl GameState {
    pub fn new(
        run_config: Option<RunConfig>,
        screen_size: (f32, f32),
        context: &mut Context,
    ) -> GameResult<GameState> {
        let conf = run_config.unwrap_or_default();
        let (send_to_game, receive_from_chat) = channel::<ChatMessage>();
        let (send_to_chat, receive_from_game) = channel::<String>();

        if conf.test_bot_chatters > 0 {
            chat_test_mock::run(
                send_to_game.clone(),
                conf.test_bot_chatters,
                conf.test_command_occurences,
                SPLASH_DURATION,
                250,
                1500,
            );
        }

        if conf.attach_to_twitch_channel {
            let _twitchchat_thread = thread::spawn(move || {
                twitch_chat_wrapper::run(receive_from_game, send_to_game).unwrap();
            });
        }
        let _ = send_to_chat.send(String::from("Chat vs. Streamer game started! Use the commands on screen to drop objects that the streamer will attempt to avoid. You get 1 point for every object you drop and 10 points for every time you hit the player!"));
        let interface =
            Interface::new(context, LIVES, crate::DROP_ZONE_COUNT, SPLASH_DURATION)?;

        // create player
        let player_scale = 4.0;
        let player_forward_sprite = Sprite::new(context, "/player_forward.png", 8, 1);
        let player_left_sprite = Sprite::new(context, "/player_left.png", 8, 1);
        let player_draw_system =
            PlayerDrawSystem::new(player_left_sprite, player_forward_sprite, player_scale);
        let player_size = player_draw_system.get_size().unwrap_or((50.0, 50.0));
        let (send_player_hit_object_event, receive_player_hit_object_event) = channel();
        let player_physics_system = PlayerPhysics::new(context, send_player_hit_object_event);
        let player = GameObject::new(
            250.0,
            250.0,
            Some(Box::new(player_draw_system)),
            player_size.0,
            player_size.1,
            Some(Box::new(player_physics_system)),
            true,
            None,
            GameObjectType::Player,
            Some(Box::new(PlayerLifeSystem::new())),
        );

        let game_objects = vec![player];

        let game_start_time = Instant::now();

        Ok(GameState {
            send_to_chat,
            receive_from_chat,
            screen_size,
            interface,
            game_objects,
            player_hit_object_event: receive_player_hit_object_event,
            running_state: RunningState::StartingSoon,
            credits: None,
            game_start_time,
            object_sound: audio::Source::new(context, "/threeTone1.ogg").unwrap(),
            scores: HashMap::new(),
            command_parser: CommandParser::new(&command::COMMAND_MAPPING),
        })
    }

    fn handle_command(
        &mut self,
        command: Option<CommandInstance>,
        context: &mut Context,
    ) -> GameResult<()> {
        if let Some(command) = command {
            let chatter = command.chatter.clone();
            self.object_sound.play().unwrap();
            self.game_objects.push(command.handle(
                self.interface.get_column_coordinates_by_index(command.id),
                context,
            )?);
            let score = self.scores.entry(chatter.name).or_insert(0);
            *score += 1;
        }
        Ok(())
    }

    fn get_player(&self) -> Option<&GameObject> {
        self.game_objects
            .iter()
            .find(|game_object| game_object.my_type == GameObjectType::Player)
    }

    fn update_scores(&self, high_scores: &mut HashMap<String, u128>) {
        for (username, score) in &self.scores {
            let high_score = high_scores.entry(username.to_owned()).or_insert(0);
            *high_score += *score;
        }
        dbg!("updating scores");
    }
}
impl EventHandler for GameState {
    fn update(&mut self, context: &mut Context) -> GameResult {
        if let Ok(chat_message) = self.receive_from_chat.try_recv() {
            if matches!(self.running_state, RunningState::Playing) {
                let chatter_name = if let Some(display_name) = chat_message.display_name {
                    display_name
                } else {
                    chat_message.name.clone()
                };
                match self.command_parser.parse_message(
                    &chat_message.message,
                    Chatter::new(
                        chatter_name,
                        chat_message.color_rgb,
                        chat_message.subscriber,
                    ),
                ) {
                    Err(error) => self.send_to_chat.send(error.to_owned()).unwrap(),
                    Ok(command) => self.handle_command(command, context)?,
                }
            }
        }

        while timer::check_update_time(context, FRAMERATE_TARGET) {
            match self.running_state {
                RunningState::StartingSoon => {
                    if let Err(error) = self.interface.update(context, LIVES) {
                        eprintln!("Error updating game objects in interface: {}", error);
                    }
                    if self.interface.splash_is_done() {
                        self.running_state = RunningState::Playing;
                        self.interface.set_timer(
                            context,
                            Instant::now(),
                            GAME_TIME,
                            (1.0, 0.0, 0.0, 1.0),
                        );
                        self.game_start_time = Instant::now();
                    }
                }
                RunningState::Playing => {
                    // get the player lives left
                    let lives_left = if let Some(player) = self.get_player() {
                        player.get_lives_left().unwrap_or(3)
                    } else {
                        0
                    };

                    if lives_left == 0 {
                        self.running_state = RunningState::ChatWon;
                    }

                    if let Err(error) = self.interface.update(context, lives_left) {
                        eprintln!("Error updating game objects in interface: {}", error);
                    }

                    let game_time_left =
                        GAME_TIME.as_secs() - self.game_start_time.elapsed().as_secs();
                    if game_time_left == 0 {
                        self.running_state = RunningState::PlayerWon;
                    }

                    let arena_size = (
                        self.screen_size.0 - self.interface.sidebar_width,
                        self.screen_size.1,
                    );

                    let collidable_game_objects: Vec<GameObject> = self
                        .game_objects
                        .clone()
                        .into_iter()
                        .filter(|game_object| game_object.collidable)
                        .collect();

                    self.game_objects.iter_mut().for_each(|game_object| {
                        if let Err(error) = game_object.update(
                            timer::time_since_start(context),
                            arena_size,
                            context,
                            &collidable_game_objects,
                        ) {
                            eprintln!("error running update: {}", error)
                        }
                    });

                    self.game_objects
                        .retain(|game_object| game_object.is_alive());

                    if let Ok(chatter) = self.player_hit_object_event.try_recv() {
                        let message_to_chat = format!("Hit! {} gets 10 points", &chatter.name);
                        let _ = self.send_to_chat.send(message_to_chat);
                        let score = self.scores.entry(chatter.name).or_insert(0);
                        *score += 10;
                    }

                    if self
                        .game_objects
                        .iter()
                        .find(|game_object| game_object.my_type == GameObjectType::Player)
                        .is_none()
                    {
                        self.running_state = RunningState::ChatWon;
                    }
                }
                RunningState::ChatWon | RunningState::PlayerWon => {
                    if let Some(credits) = &mut self.credits {
                        if !credits.update() {
                            ggez::event::quit(context);
                        }
                    } else {
                        let mut high_scores = utilities::load_scores(SCORES_FILE_NAME, context);
                        self.update_scores(&mut high_scores);
                        if let Err(error) =
                            utilities::save_scores(context, SCORES_FILE_NAME, &high_scores)
                        {
                            eprintln!("Error saving high scores to disk: {}", error);
                        }
                        self.credits = Some(Credits::new(
                            self.running_state,
                            context,
                            self.screen_size,
                            &high_scores,
                            &self.scores,
                        )?);
                    }
                }
            }
        }
        Ok(())
    }

    fn draw(&mut self, context: &mut Context) -> GameResult {
        graphics::clear(context, BLACK);

        self.interface.draw(context, &self.running_state)?;

        match self.running_state {
            RunningState::StartingSoon => (),
            RunningState::Playing => {
                for game_object in self.game_objects.iter() {
                    game_object.draw(context)?;
                }
            }
            RunningState::PlayerWon | RunningState::ChatWon => {
                if let Some(credits) = &self.credits {
                    credits.draw(context)?;
                }
            }
        }

        graphics::present(context)
    }

    fn resize_event(&mut self, ctx: &mut Context, width: f32, height: f32) {
        self.screen_size = (width, height);

        let _ = graphics::set_screen_coordinates(
            ctx,
            graphics::Rect {
                x: 0.0,
                y: 0.0,
                w: width,
                h: height,
            },
        );
        self.interface.update_screen_size(ctx, width, height);
    }
}