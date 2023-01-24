use crate::prelude::*;

use std::{sync::Weak, process::Termination};
use poise::async_trait;

#[async_trait]
pub trait BotTask: Send + Sync {
    type Data: Send;
    type Term: Termination;

    async fn setup(&self, framework: Weak<poise::Framework<Data, Error>>) -> Option<Self::Data>;
    async fn run(self, data: Self::Data) -> Self::Term;
}
