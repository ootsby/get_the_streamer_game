use std::collections::HashMap;

use super::utilities;
use crate::running_state::RunningState;
use ggez::{
    graphics::DrawParam,
    graphics::Font,
    graphics::Scale,
    graphics::{self, Text},
    nalgebra::Point2,
    Context, GameResult,
};
use rand::prelude::*;
use rand::seq::IteratorRandom;

const VELOCITY_Y: f32 = -2.0;
const DEFAULT_STREAMER_WIN_MESSAGE: &str = "Streamer won!";
const CONTRIBUTORS: [&str; 6] = [
    "brookspatton",
    "dmb1107",
    "LordMZTE",
    "MeirKlemp",
    "ootsby",
    "dannyfritz",
];

pub struct Credits {
    all_credits: Vec<(Text, Point2<f32>)>,
}

impl Credits {
    pub fn new(
        running_state: RunningState,
        context: &mut Context,
        screen_size: (f32, f32),
        high_scores: &HashMap<String, u128>,
        scores: &HashMap<String, u128>,
    ) -> GameResult<Self> {
        let file_name = if matches!(running_state, RunningState::PlayerWon) {
            "streamer_wins_messages.txt"
        } else {
            "chat_wins_messages.txt"
        };
        let snarky_message = Self::get_random_message(file_name);
        let mut credit_y = screen_size.1;

        let mut all_credits = vec![];
        all_credits.push(Self::create_game_over_text(context, screen_size));
        Self::create_credit(
            context,
            screen_size,
            &snarky_message,
            Some(75.0),
            &mut all_credits,
            &mut credit_y,
        );
        Self::create_space(context, screen_size, &mut all_credits, &mut credit_y);
        Self::create_credit(
            context,
            screen_size,
            "Chatters (score) -> (total)",
            None,
            &mut all_credits,
            &mut credit_y,
        );

        scores.iter().for_each(|(username, score)| {
            let high_score = high_scores.get(username).unwrap_or(score);
            let title = format!("{} - {} -> {}", username, score, high_score);
            Self::create_credit(
                context,
                screen_size,
                &title,
                None,
                &mut all_credits,
                &mut credit_y,
            )
        });

        Self::create_credit(
            context,
            screen_size,
            "Built Live on Twitch at https://www.twitch.tv/brookzerker",
            None,
            &mut all_credits,
            &mut credit_y,
        );

        Self::create_space(context, screen_size, &mut all_credits, &mut credit_y);

        Self::create_credit(
            context,
            screen_size,
            "Built By",
            None,
            &mut all_credits,
            &mut credit_y,
        );

        CONTRIBUTORS.iter().for_each(|&contributor| {
            Self::create_credit(
                context,
                screen_size,
                contributor,
                None,
                &mut all_credits,
                &mut credit_y,
            );
        });

        Self::create_space(context, screen_size, &mut all_credits, &mut credit_y);

        Ok(Credits { all_credits })
    }

    fn get_random_message(file_name: &str) -> String {
        let mut rng = thread_rng();
        if let Some(messages) = utilities::load_messages(file_name) {
            messages
                .choose(&mut rng)
                .unwrap_or_else(|| DEFAULT_STREAMER_WIN_MESSAGE.to_owned())
        } else {
            DEFAULT_STREAMER_WIN_MESSAGE.to_owned()
        }
    }

    fn create_credit(
        context: &mut Context,
        (screen_width, _): (f32, f32),
        title: &str,
        font_scale: Option<f32>,
        all_credits: &mut Vec<(Text, Point2<f32>)>,
        y: &mut f32,
    ) {
        let mut text = Text::new(title);
        let scale = font_scale.unwrap_or(50.0);
        text.set_font(Font::default(), Scale::uniform(scale));
        let text_size = text.dimensions(context);
        let location = Point2::new(screen_width / 2.0 - text_size.0 as f32 / 2.0, *y);
        all_credits.push((text, location));
        *y += text_size.1 as f32 * 2.0;
    }

    fn create_game_over_text(
        context: &mut Context,
        (screen_width, screen_height): (f32, f32),
    ) -> (Text, Point2<f32>) {
        let mut game_over_text = Text::new("Game Over");
        game_over_text.set_font(Font::default(), Scale::uniform(50.0));
        let (game_over_width, game_over_height) = game_over_text.dimensions(context);
        let game_over_location = Point2::new(
            screen_width / 2.0 - game_over_width as f32 / 2.0,
            screen_height / 2.0 - game_over_height as f32 / 2.0,
        );
        (game_over_text, game_over_location)
    }

    fn create_space(
        context: &mut Context,
        screen_size: (f32, f32),
        all_credits: &mut Vec<(Text, Point2<f32>)>,
        credit_y: &mut f32,
    ) {
        Self::create_credit(
            context,
            screen_size,
            "---",
            Some(100.0),
            all_credits,
            credit_y,
        );
    }

    pub fn update(&mut self) -> bool {
        for credit in &mut self.all_credits {
            credit.1.y += VELOCITY_Y;
        }

        self.all_credits.retain(|credit| credit.1.y > -100.0);

        !self.all_credits.is_empty()
    }

    pub fn draw(&self, context: &mut Context) -> GameResult<()> {
        self.all_credits.iter().try_for_each(|credit| {
            graphics::draw(context, &credit.0, DrawParam::new().dest(credit.1))
        })
    }
}
