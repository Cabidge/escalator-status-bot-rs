use crate::{prelude::*, bot_tasks::BotTask, data::report::UserReport, generate};

use futures::future::join_all;
use poise::async_trait;
use std::sync::Arc;
use tokio::sync::broadcast::{self, error::RecvError};

pub struct SyncTask;

pub struct TaskData {
    pool: sqlx::PgPool,
    reports: broadcast::Receiver<UserReport>,
    cache_http: Arc<serenity::CacheAndHttp>,
}

#[async_trait]
impl BotTask for SyncTask {
    type Data = TaskData;
    type Term = anyhow::Result<()>;

    async fn setup(
        &self,
        framework: std::sync::Weak<poise::Framework<Data, Error>>,
    ) -> Option<Self::Data> {
        let framework = framework.upgrade()?;

        let cache_http = Arc::clone(&framework.client().cache_and_http);

        let data = framework.user_data().await;

        Some(TaskData {
            pool: data.pool.clone(),
            reports: data.receiver(),
            cache_http,
        })
    }

    async fn run(self, mut data: Self::Data) -> Self::Term {
        sync_menus(&data).await?;

        loop {
            match data.reports.recv().await {
                Ok(report) if report.affected_escalators.is_empty() => continue,
                Ok(_) | Err(RecvError::Lagged(_)) => (),
                Err(RecvError::Closed) => return Ok(()),
            }

            sync_menus(&data).await?;
        }
    }
}

async fn sync_menus(data: &TaskData) -> Result<(), sqlx::Error> {
    let menus = sqlx::query_as::<_, (i64, i64)>(
        "
        SELECT channel_id, message_id
        FROM menu_messages
        "
    )
    .fetch_all(&data.pool)
    .await?;

    if menus.is_empty() {
        log::debug!("No menu messages to sync, skipping.");
        return Ok(());
    }

    let statuses = generate::menu_status(&data.pool).await?;

    let mut builder = serenity::EditMessage::default();

    builder.content(statuses)
        .set_components(generate::menu_buttons());

    let map = Arc::new(serenity::json::Value::from(serenity::json::hashmap_to_json_map(builder.0)));

    let update_all = menus.into_iter()
        .map(|(channel_id, message_id)| {
            let http = Arc::clone(&data.cache_http.http);
            let map = Arc::clone(&map);

            async move {
                let _ = http.edit_message(channel_id as u64, message_id as u64, &map).await.ok();
            }
        });

    join_all(update_all).await;

    Ok(())
}
