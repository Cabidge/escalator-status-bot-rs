use std::sync::Arc;

use tokio::sync::broadcast::{self, error::RecvError};

use crate::{
    data::{ReportKind, Update, UserReport},
    prelude::*,
};

use super::BotTask;

pub struct AlertTask(pub broadcast::Receiver<Update>);

impl BotTask for AlertTask {
    fn begin(
        mut self,
        framework: std::sync::Arc<poise::Framework<Data, Error>>,
    ) -> tokio::task::JoinHandle<()> {
        let cache_http = Arc::clone(&framework.client().cache_and_http);

        tokio::spawn(async move {
            let alerts = &framework.user_data().await.alerts;

            loop {
                let report = match self.0.recv().await {
                    Ok(Update::Report {
                        report,
                        kind: ReportKind::Normal,
                    }) => report,
                    Ok(update) => {
                        log::debug!("Received {update:?}, skipping...");
                        continue;
                    }
                    Err(RecvError::Lagged(n)) => {
                        log::error!("Update receiver lagged by {n} updates.");
                        continue;
                    }
                    Err(RecvError::Closed) => {
                        log::warn!("Update receiver has closed.");
                        break;
                    }
                };

                log::info!("Alerting users...");

                let UserReport {
                    escalators, status, ..
                } = report;

                alerts
                    .lock()
                    .await
                    .alert(&cache_http, escalators, status)
                    .await;
            }
        })
    }
}
