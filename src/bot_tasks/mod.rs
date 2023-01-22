use crate::prelude::*;

use std::sync::Weak;
use tokio::task::JoinHandle;

pub trait BotTask {
    fn begin(self, framework: Weak<poise::Framework<Data, Error>>) -> JoinHandle<()>;
}
