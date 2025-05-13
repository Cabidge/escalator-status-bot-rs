pub mod channels;
pub mod escalator;
pub mod escalator_input;
pub mod report;
pub mod status;

use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct Data {
    pub pool: sqlx::PgPool,
    channels: Arc<parking_lot::RwLock<channels::AnyChannels>>,
}

impl Data {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self {
            pool,
            channels: Arc::new(parking_lot::RwLock::new(channels::AnyChannels::new())),
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
