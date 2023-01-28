pub mod channels;
pub mod escalator;
pub mod escalator_input;
pub mod report;
pub mod status;

use crate::prelude::*;

use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

pub struct Data {
    pub shard_manager: Arc<Mutex<serenity::ShardManager>>,
    pub pool: sqlx::PgPool,
    channels: parking_lot::RwLock<channels::AnyChannels>,
}

impl Data {
    pub fn new(shard_manager: Arc<Mutex<serenity::ShardManager>>, pool: sqlx::PgPool) -> Self {
        Self {
            shard_manager,
            pool,
            channels: parking_lot::RwLock::new(channels::AnyChannels::new()),
        }
    }

    /// Attempts to send a value, not creating a Sender if no Receivers exist.
    pub fn send_message<T: 'static + Clone + Send + Sync>(&self, value: T) {
        let _ = self.channels.read().try_send(value).ok();
    }

    pub fn send_message_with<T: 'static + Clone + Send + Sync, F>(&self, f: F)
    where
        F: FnOnce() -> T,
    {
        if let Some(sender) = self.channels.read().try_sender() {
            let value = f();
            let _ = sender.send(value).ok();
        }
    }

    pub fn sender<T: 'static + Clone + Send + Sync>(&self) -> broadcast::Sender<T> {
        self.channels.write().sender()
    }

    pub fn receiver<T: 'static + Clone + Send + Sync>(&self) -> broadcast::Receiver<T> {
        self.channels.write().receiver()
    }
}
