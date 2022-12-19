use std::{sync::Arc, time::Duration};

use crate::{data::Update, prelude::*};

use tokio::{sync::broadcast, task::JoinHandle, time::Instant};

/// How much time to wait to send the history after receiving an update
/// if there hasn't been one in the past MAX_INTERVAL.
const MIN_INTERVAL: Duration = Duration::from_secs(60);

/// The most amount of time to wait to send the history after receiving an update.
const MAX_INTERVAL: Duration = Duration::from_secs(5 * 60);

pub fn create_task(
    framework: Arc<poise::Framework<Data, Error>>,
    mut updates: broadcast::Receiver<Update>,
) -> JoinHandle<()> {
    let cache_http = Arc::clone(&framework.client().cache_and_http);

    tokio::spawn(async move {
        let data = framework.user_data().await;
        let history_channel = &data.history_channel;

        let mut last_announcement = Instant::now();

        loop {
            // wait until an update is received
            let first_update = match updates.recv().await {
                Ok(update) => update,
                // if the channel closed (for some reason) then stop the loop
                Err(broadcast::error::RecvError::Closed) => break,
                // if the receiver is lagging beind, restart the loop and try receiving again
                // TODO: log channel lag
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
            };

            let now = Instant::now();

            let time_since_last = now.duration_since(last_announcement);
            let delay = if time_since_last + MIN_INTERVAL < MAX_INTERVAL {
                MAX_INTERVAL - time_since_last
            } else {
                MIN_INTERVAL
            };

            let sleep = tokio::time::sleep(delay);
            tokio::pin!(sleep);

            // continue to save the updates until the interval is up
            let mut history = vec![first_update];
            loop {
                tokio::select! {
                    Ok(next_update) = updates.recv() => {
                        history.push(next_update);
                        continue;
                    }
                    () = &mut sleep => break,
                }
            }

            // TODO: format announcement
            if let Err(err) = history_channel
                .read()
                .await
                .send(&cache_http, |msg| msg.content(format!("{history:#?}")))
                .await
            {
                // TODO: handle error
                println!("{err:?}");
            }

            last_announcement = Instant::now();
        }
    })
}
