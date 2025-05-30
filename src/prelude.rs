pub use poise::serenity_prelude as serenity;

pub use crate::data::Data;

pub use crate::data::escalator::{Escalator, EscalatorFloors};

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;
