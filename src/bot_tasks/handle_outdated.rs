use crate::prelude::*;

use std::{sync::Arc, time::Duration};
use tokio::task::JoinHandle;

/// How often to check for outdated statuses.
const CHECK_INTERVALS: Duration = Duration::from_secs(10 * 60);

pub fn begin_task(framework: Arc<poise::Framework<Data, Error>>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let data = framework.user_data().await;
        let statuses = &data.statuses;

        let mut interval = tokio::time::interval(CHECK_INTERVALS);

        loop {
            interval.tick().await;
            statuses.lock().await.handle_outdated();
        }
    })
}
