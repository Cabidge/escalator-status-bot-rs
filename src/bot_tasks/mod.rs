pub mod alert;
pub mod announce;
pub mod menus;

use crate::prelude::*;

use poise::async_trait;
use std::{process::Termination, sync::Weak};

#[async_trait]
pub trait BotTask: Send + Sync {
    type Data: Send;
    type Term: Termination;

    async fn setup(&self, framework: Weak<poise::Framework<Data, Error>>) -> Option<Self::Data>;
    async fn run(self, data: Self::Data) -> Self::Term;
}
