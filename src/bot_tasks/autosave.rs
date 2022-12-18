use std::{time::Duration, sync::Arc};

use crate::prelude::*;

use shuttle_persist::PersistInstance;
use tokio::task::JoinHandle;

/// How much time to wait between each time trying to save.
const AUTO_SAVE_INTERVAL: Duration = Duration::from_secs(15 * 60);

pub fn begin_task(framework: Arc<poise::Framework<Data, Error>>, persist: PersistInstance) -> JoinHandle<()> {
    tokio::spawn(async move {
        let data = framework.user_data().await.clone_arcs();
        let mut interval = tokio::time::interval(AUTO_SAVE_INTERVAL);

        loop {
            interval.tick().await;

            // TODO: proper logging
            println!("Attempting save...");
            data.save_persist(&persist).await;
        }
    })
}
