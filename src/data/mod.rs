pub mod alerts;
pub mod escalator_input;
pub mod escalators;
pub mod history_channel;
pub mod report_menu;
pub mod status;

pub use escalators::*;
pub use history_channel::HistoryChannel;
pub use report_menu::ReportMenu;

use crate::prelude::*;

use shuttle_persist::PersistInstance;
use std::{fmt::Display, sync::Arc};
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};

use self::{escalator_input::EscalatorInput, status::Status, alerts::Alerts};

#[derive(Debug)]
pub struct Data {
    pub shard_manager: Arc<Mutex<serenity::ShardManager>>,
    pub statuses: Arc<Mutex<Statuses>>,
    pub report_menu: Arc<Mutex<ReportMenu>>,
    pub history_channel: Arc<RwLock<HistoryChannel>>,
    pub alerts: Arc<Mutex<Alerts>>,
}

#[derive(Debug, Clone, Copy)]
pub struct UserReport {
    pub escalators: EscalatorInput,
    pub status: Status,
    pub reporter: Option<serenity::UserId>,
}

impl Data {
    pub async fn load_persist(
        persist: &PersistInstance,
        shard_manager: Arc<Mutex<serenity::ShardManager>>,
        ctx: &serenity::Context,
        user_reports_tx: mpsc::Sender<UserReport>,
        updates_tx: broadcast::Sender<Update>,
    ) -> Self {
        let statuses = Statuses::load_persist(persist, updates_tx);
        let statuses = Arc::new(Mutex::new(statuses));

        let report_menu = ReportMenu::load_persist(persist, user_reports_tx, ctx).await;
        let report_menu = Arc::new(Mutex::new(report_menu));

        let history_channel = HistoryChannel::load_persist(persist);
        let history_channel = Arc::new(RwLock::new(history_channel));

        let alerts = Alerts::load_persist(persist, &ctx).await;
        let alerts = Arc::new(Mutex::new(alerts));

        Data {
            shard_manager,
            statuses,
            report_menu,
            history_channel,
            alerts,
        }
    }

    pub async fn save_persist(&self, persist: &PersistInstance) {
        self.statuses.lock().await.save_persist(persist);
        self.report_menu.lock().await.save_persist(persist);
        self.history_channel.write().await.save_persist(persist);
        self.alerts.lock().await.save_persist(persist);
    }
}

impl Display for UserReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let emoji = self.status.emoji();
        let reporter = self
            .reporter
            .map(|id| format!("<@{}>", id))
            .unwrap_or_else(|| String::from("an unknown user"));

        write!(
            f,
            "`{emoji}` {reporter} reported {}.",
            self.escalators.message_noun()
        )
    }
}
