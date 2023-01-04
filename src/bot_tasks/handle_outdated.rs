use crate::prelude::*;

use std::{sync::Arc, time::Duration};
use tokio::task::JoinHandle;

use super::BotTask;

/// How often to check for outdated statuses.
const CHECK_INTERVALS: Duration = Duration::from_secs(10 * 60);

pub struct HandleOutdatedTask;

impl BotTask for HandleOutdatedTask {
    fn begin(self, framework: Arc<poise::Framework<Data, Error>>) -> JoinHandle<()> {
        tokio::spawn(async move {
            let data = framework.user_data().await;
            let statuses = &data.statuses;

            let mut interval = tokio::time::interval(CHECK_INTERVALS);

            loop {
                interval.tick().await;

                log::info!("Checking for outdated statuses...");
                statuses.lock().await.handle_outdated();
            }
        })
    }
}
