use crate::prelude::*;

use std::sync::Weak;
use tokio::task::JoinHandle;

pub trait BotTask {
    type Data;

    fn setup(&self, framework: Weak<poise::Framework<Data, Error>>) -> Option<Data>;
    fn begin(self, data: Self::Data) -> JoinHandle<()>;
}
