mod announcements;
mod autosave;
mod forward_reports;
mod handle_outdated;
mod sync_menu;

pub use announcements::AnnouncementTask;
pub use autosave::AutoSaveTask;
pub use forward_reports::ForwardReportTask;
pub use handle_outdated::HandleOutdatedTask;
pub use sync_menu::SyncMenuTask;

use crate::prelude::*;

use tokio::task::JoinHandle;
use std::sync::Arc;

pub trait BotTask {
    fn begin(self, framework: Arc<poise::Framework<Data, Error>>) -> JoinHandle<()>;
}
