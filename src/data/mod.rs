pub mod channels;
pub mod escalator;
pub mod escalator_input;
pub mod menu;
pub mod report;
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

    /// Attempts to send a value, not creating a Sender if no Receivers exist.
    pub fn send_message<T: 'static + Clone + Send + Sync>(&self, value: T) {
        let _ = self.channels.lock().try_send(value).ok();
    }

    pub fn sender<T: 'static + Clone + Send + Sync>(&self) -> broadcast::Sender<T> {
        self.channels.lock().sender()
    }

    pub fn receiver<T: 'static + Clone + Send + Sync>(&self) -> broadcast::Receiver<T> {
        self.channels.lock().receiver()
    }
}
