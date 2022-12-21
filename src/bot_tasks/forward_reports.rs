use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::data::UserReport;
use crate::prelude::*;

pub fn begin_task(
    framework: Arc<poise::Framework<Data, Error>>,
    mut user_reports_rx: mpsc::Receiver<UserReport>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let data = framework.user_data().await;

        while let Some(report) = user_reports_rx.recv().await {
            data.statuses.lock().await.report(report);
        }
    })
}
