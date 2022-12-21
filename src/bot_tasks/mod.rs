pub mod announcements;
pub mod autosave;
pub mod forward_reports;
pub mod handle_outdated;
pub mod sync_menu;

use tokio::task::JoinHandle;

use crate::prelude::*;
use std::sync::Arc;

pub trait BotTask {
    fn begin(self, framework: Arc<poise::Framework<Data, Error>>) -> JoinHandle<()>;
}
