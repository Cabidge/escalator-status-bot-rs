use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::data::UserReport;
use crate::prelude::*;

use super::BotTask;

pub struct ForwardReportTask(pub mpsc::Receiver<UserReport>);

impl BotTask for ForwardReportTask {
    fn begin(mut self, framework: Arc<poise::Framework<Data, Error>>) -> JoinHandle<()> {
        tokio::spawn(async move {
            let data = framework.user_data().await;

            while let Some(report) = self.0.recv().await {
                log::info!("Forwarding reports...");
                data.statuses.lock().await.report(report);
            }
        })
    }
}
