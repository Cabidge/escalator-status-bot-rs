use std::{sync::Arc, time::Duration};

use crate::prelude::*;

use shuttle_persist::PersistInstance;
use tokio::task::JoinHandle;

use super::BotTask;

/// How much time to wait between each time trying to save.
const AUTO_SAVE_INTERVAL: Duration = Duration::from_secs(60);

pub struct AutoSaveTask(pub Arc<PersistInstance>);

impl BotTask for AutoSaveTask {
    fn begin(self, framework: Arc<poise::Framework<Data, Error>>) -> JoinHandle<()> {
        tokio::spawn(async move {
            let data = framework.user_data().await;
            let mut interval = tokio::time::interval(AUTO_SAVE_INTERVAL);

            loop {
                interval.tick().await;

                // TODO: proper logging
                println!("Attempting save...");
                data.save_persist(&self.0).await;
            }
        })
    }
}
