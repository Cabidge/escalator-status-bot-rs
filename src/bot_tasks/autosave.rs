use std::time::Duration;

use crate::prelude::*;

use shuttle_persist::PersistInstance;
use tokio::task::JoinHandle;

/// How much time to wait between each time trying to save.
const AUTO_SAVE_INTERVAL: Duration = Duration::from_secs(15 * 60);

pub fn begin_task(persist: PersistInstance, data: &Data) -> JoinHandle<()> {
    let data = data.clone_arcs();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(AUTO_SAVE_INTERVAL);

        loop {
            interval.tick().await;

            // TODO: proper logging
            println!("Attempting save...");
            data.save_persist(&persist).await;
        }
    })
}
