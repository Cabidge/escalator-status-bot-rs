pub mod escalators;
pub mod history_channel;
pub mod report_menu;

pub use escalators::*;
pub use history_channel::HistoryChannel;
pub use report_menu::ReportMenu;

use shuttle_persist::PersistInstance;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex, RwLock};

pub struct Data {
    pub statuses: Arc<Mutex<Statuses>>,
    pub report_menu: Arc<Mutex<ReportMenu>>,
    pub history_channel: Arc<RwLock<HistoryChannel>>,
}

impl Data {
    /// Create a clone of Data by cloning all of the Arcs.
    ///
    /// Not deriving Clone for Data because this is more explicit.
    pub fn clone_arcs(&self) -> Data {
        let statuses = Arc::clone(&self.statuses);
        let report_menu = Arc::clone(&self.report_menu);
        let history_channel = Arc::clone(&self.history_channel);

        Data { statuses, report_menu, history_channel }
    }

    pub fn load_persist(persist: &PersistInstance) -> (Self, broadcast::Receiver<Update>) {
        let (statuses, update_rx) = Statuses::load_persist(persist);
        let statuses = Arc::new(Mutex::new(statuses));

        let report_menu = ReportMenu::load_persist(persist);
        let report_menu = Arc::new(Mutex::new(report_menu));

        let history_channel = HistoryChannel::load_persist(persist);
        let history_channel = Arc::new(RwLock::new(history_channel));

        let data = Data {
            statuses,
            report_menu,
            history_channel,
        };

        (data, update_rx)
    }

    pub async fn save_persist(&self, persist: &PersistInstance) {
        self.statuses.lock().await.save_persist(persist);
        self.report_menu.lock().await.save_persist(persist);
        self.history_channel.write().await.save_persist(persist);
    }
}
