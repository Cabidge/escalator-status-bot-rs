use crate::{data::report::UserReport, generate, prelude::*};

use super::BotTask;

use futures::future::join_all;
use poise::serenity_prelude::{CacheHttp, CreateMessage};
use std::{sync::Arc, time::Duration};
use tokio::sync::broadcast;

pub struct AnnounceTask {
    delay: Duration,
    max_reports_displayed: usize,
}

pub struct TaskData<T> {
    pool: sqlx::PgPool,
    reports: broadcast::Receiver<UserReport>,
    cache_http: Arc<T>,
}

impl Default for AnnounceTask {
    fn default() -> Self {
        Self {
            delay: Duration::from_secs(2 * 60),
            max_reports_displayed: 8,
        }
    }
}

impl<T: CacheHttp + 'static> BotTask<T> for AnnounceTask {
    type Data = TaskData<T>;
    type Term = anyhow::Result<()>;

    async fn setup(&self, data: &Data, cache_http: Arc<T>) -> Option<Self::Data> {
        Some(TaskData {
            pool: data.pool.clone(),
            reports: data.receiver(),
            cache_http,
        })
    }

    async fn run(self, mut data: Self::Data) -> Self::Term {
        loop {
            let reports = self.accumulate_reports(&mut data.reports).await?;

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
            let embed = generate::gist(&data.pool).await?;

            let reports = generate::announcement(self.max_reports_displayed, reports.iter().rev());

            let embed = embed.timestamp(chrono::Utc::now()).field(
                "Recent reports (newest first)",
                reports,
                false,
            );

            // send embed to history channel
            log::info!("Sending announcement...");

            let send_all = channels.into_iter()
                .map(|(channel_id,)| {
                    let channel = serenity::ChannelId::new(channel_id as u64);
                    let embed = embed.clone();
                    let cache_http = Arc::clone(&data.cache_http);

                    async move {
                        let msg = CreateMessage::new().embed(embed);
                        let res = channel.send_message(&cache_http, msg)
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

        log::info!(
            "Received update, pooling for {} seconds before announcing.",
            self.delay.as_secs()
        );

        let sleep = tokio::time::sleep(self.delay);
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
