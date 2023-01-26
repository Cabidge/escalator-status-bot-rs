use crate::{data::report::UserReport, generate, prelude::*};

use super::BotTask;

use futures::future::join_all;
use poise::async_trait;
use std::{sync::Arc, time::Duration};
use tokio::{sync::broadcast, time::Instant};

pub struct AnnounceTask {
    min_interval: Duration,
    max_interval: Duration,
    max_reports_displayed: usize,
}

pub struct TaskData {
    pool: sqlx::PgPool,
    reports: broadcast::Receiver<UserReport>,
    cache_http: Arc<serenity::CacheAndHttp>,
}

impl Default for AnnounceTask {
    fn default() -> Self {
        Self {
            min_interval: Duration::from_secs(60),
            max_interval: Duration::from_secs(5 * 60),
            max_reports_displayed: 8,
        }
    }
}

#[async_trait]
impl BotTask for AnnounceTask {
    type Data = TaskData;
    type Term = anyhow::Result<()>;

    async fn setup(
        &self,
        framework: std::sync::Weak<poise::Framework<Data, Error>>,
    ) -> Option<Self::Data> {
        let framework = framework.upgrade()?;

        let data = framework.user_data().await;

        let cache_http = Arc::clone(&framework.client().cache_and_http);

        Some(TaskData {
            pool: data.pool.clone(),
            reports: data.receiver(),
            cache_http,
        })
    }

    async fn run(self, mut data: Self::Data) -> Self::Term {
        loop {
            let last_announcement = Instant::now();

            let reports = self
                .accumulate_reports(last_announcement, &mut data.reports)
                .await?;

            log::info!("Grabbing announcement channels...");

            let channels = sqlx::query_as::<_, (i64,)>(
                "
                SELECT channel_id
                FROM announcement_channels
                ",
            )
            .fetch_all(&data.pool)
            .await?;

            if channels.is_empty() {
                log::info!("No announcement channels found, skipping announcement generation.");
                continue;
            }

            log::info!("Generating announcement...");

            // get summary of the current escalator statuses
            let mut embed = generate::gist(&data.pool).await?;

            let reports = generate::announcement(self.max_reports_displayed, reports.into_iter().rev());

            embed.timestamp(chrono::Utc::now()).field(
                "Recent reports (newest first)",
                reports,
                false,
            );

            // send embed to history channel
            log::info!("Sending announcement...");

            let send_all = channels.into_iter()
                .map(|(channel_id,)| {
                    let channel = serenity::ChannelId(channel_id as u64);
                    let embed = embed.clone();
                    let cache_http = Arc::clone(&data.cache_http);

                    async move {
                        let res = channel.send_message(&cache_http.http, move |msg| msg.set_embed(embed))
                            .await;

                        match res {
                            Ok(msg) => {
                                let Ok(channel) = msg.channel(&cache_http)
                                    .await else { return };

                                let Some(channel) = channel.guild() else { return };

                                if channel.kind == serenity::ChannelType::News {
                                    let _ = msg.crosspost(&cache_http).await.ok();
                                }
                            }
                            Err(err) => log::warn!("An error ocurred trying to send a message in the channel <#{channel_id}>: {err}"),
                        }
                    }
                });

            join_all(send_all).await;
        }
    }
}

impl AnnounceTask {
    async fn accumulate_reports(
        &self,
        last_announcement: Instant,
        reports: &mut broadcast::Receiver<UserReport>,
    ) -> anyhow::Result<Vec<UserReport>> {
        let mut accumulated = loop {
            match reports.recv().await {
                Ok(report) => {
                    break vec![report];
                }
                // if the channel closed (for some reason) then stop the loop
                Err(broadcast::error::RecvError::Closed) => {
                    anyhow::bail!("Update receiver has closed.");
                }
                // if the receiver is lagging beind, restart the loop and try receiving again
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    log::warn!("Update receiver has lagged by {n} updates.");
                }
            };
        };

        let now = Instant::now();

        let time_since_last = now.duration_since(last_announcement);

        // TODO: figure this out
        let delay = if self.min_interval < self.max_interval - time_since_last {
            self.max_interval - time_since_last
        } else {
            self.min_interval
        };

        log::info!(
            "Received update, pooling for {} seconds before announcing.",
            delay.as_secs()
        );

        let sleep = tokio::time::sleep(delay);
        tokio::pin!(sleep);

        // continue to save the updates until the interval is up
        loop {
            tokio::select! {
                Ok(report) = reports.recv() => accumulated.push(report),
                () = &mut sleep => break,
            }
        }

        Ok(accumulated)
    }
}
