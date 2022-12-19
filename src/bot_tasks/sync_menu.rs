use tokio::{
    sync::broadcast::{self, error::RecvError},
    task::JoinHandle,
};

use crate::{data::Update, prelude::*};

use std::sync::Arc;

pub fn begin_task(
    framework: Arc<poise::Framework<Data, Error>>,
    mut updates: broadcast::Receiver<Update>,
) -> JoinHandle<()> {
    let cache_http = Arc::clone(&framework.client().cache_and_http);

    tokio::spawn(async move {
        let data = framework.user_data().await;
        let statuses = &data.statuses;
        let report_menu = &data.report_menu;

        // wait until an update is received
        while let Ok(_) | Err(RecvError::Lagged(_)) = updates.recv().await {
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
