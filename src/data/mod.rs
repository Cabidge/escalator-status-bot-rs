pub mod channels;
pub mod escalator;
pub mod status;

use crate::prelude::*;

use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

pub struct Data {
    pub shard_manager: Arc<Mutex<serenity::ShardManager>>,
    pub pool: sqlx::PgPool,
    channels: parking_lot::Mutex<channels::AnyChannels>,
}

impl Data {
    pub fn new(shard_manager: Arc<Mutex<serenity::ShardManager>>, pool: sqlx::PgPool) -> Self {
        Self {
            shard_manager,
            pool,
            channels: parking_lot::Mutex::new(channels::AnyChannels::new()),
        }
    }

    pub fn sender<T: 'static + Clone + Send + Sync>(&mut self) -> broadcast::Sender<T> {
        self.channels.lock().sender()
    }

    pub fn receiver<T: 'static + Clone + Send + Sync>(&mut self) -> broadcast::Receiver<T> {
        self.channels.lock().receiver()
    }
}
