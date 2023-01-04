use tokio::{
    sync::broadcast::{self, error::RecvError},
    task::JoinHandle,
};

use crate::{data::{Update, ReportKind}, prelude::*};

use std::sync::Arc;

use super::BotTask;

pub struct SyncMenuTask(pub broadcast::Receiver<Update>);

impl BotTask for SyncMenuTask {
    fn begin(mut self, framework: Arc<poise::Framework<Data, Error>>) -> JoinHandle<()> {
        let cache_http = Arc::clone(&framework.client().cache_and_http);

        tokio::spawn(async move {
            let data = framework.user_data().await;
            let statuses = &data.statuses;
            let report_menu = &data.report_menu;

            loop {
                // wait until an update is received
                match self.0.recv().await {
                    // skip update if report was redundant
                    Ok(Update::Report {
                        kind: ReportKind::Redundant, ..
                    }) => continue,
                    Ok(_) => (),
                    Err(RecvError::Lagged(_)) => (),
                    Err(RecvError::Closed) => break,
                }

                let mut report_menu = report_menu.lock().await;

                // if there is no report menu message, don't bother
                if !report_menu.is_initialized() {
                    continue;
                }

                let statuses = statuses.lock().await;
                if let Err(err) = report_menu.update(&cache_http, &statuses).await {
                    // TODO: preoperly log error
                    println!("{err:?}");
                }
            }
        })
    }
}
