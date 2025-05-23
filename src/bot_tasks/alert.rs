use std::sync::Arc;

use crate::{data::report::UserReport, generate, prelude::*};

use super::BotTask;

use futures::future::join_all;
use poise::serenity_prelude::CacheHttp;
use smallvec::{smallvec, SmallVec};
use tokio::sync::broadcast;

pub struct AlertTask;

pub struct TaskData<T> {
    pool: sqlx::PgPool,
    reports: broadcast::Receiver<UserReport>,
    cache_http: Arc<T>,
}

impl<T: CacheHttp + 'static> BotTask<T> for AlertTask {
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
            let report = match data.reports.recv().await {
                Ok(report) if report.affected_escalators.is_empty() => continue,
                Ok(report) => report,
                // if the channel closed (for some reason) then stop the loop
                Err(broadcast::error::RecvError::Closed) => {
                    anyhow::bail!("Update receiver has closed.");
                }
                // if the receiver is lagging beind, restart the loop and try receiving again
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    log::warn!("Update receiver has lagged by {n} updates.");
                    continue;
                }
            };

            let mut starts: SmallVec<[_; 2]> = smallvec![];
            let mut ends: SmallVec<[_; 2]> = smallvec![];

            for &EscalatorFloors { start, end } in &report.affected_escalators {
                starts.push(start as i16);
                ends.push(end as i16);
            }

            let users = sqlx::query_as::<_, (i64,)>(
                "
                SELECT DISTINCT user_id
                FROM alerts a
                WHERE EXISTS (
                    SELECT 1
                    FROM UNNEST($1::smallint[], $2::smallint[])
                        AS r (floor_start, floor_end)
                    WHERE a.floor_start = r.floor_start
                    AND a.floor_end = r.floor_end
                )
                ",
            )
            .bind(&starts[..])
            .bind(&ends[..])
            .fetch_all(&data.pool)
            .await?;

            if users.is_empty() {
                log::info!("No users watching affected escalators, skipping.");
                continue;
            }

            let message = generate::alert(&report);

            log::info!("Sending alert messages...");

            let send_all = users.into_iter().map(|(user_id,)| {
                let message = message.clone();
                let cache_http = Arc::clone(&data.cache_http);
                let user = serenity::UserId::new(user_id as u64);
                async move {
                    let Ok(dm) = user.create_dm_channel(&cache_http).await else {
                        return;
                    };
                    let _ = dm.say(&cache_http, message).await.ok();
                }
            });

            join_all(send_all).await;
        }
    }
}
