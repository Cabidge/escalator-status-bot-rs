use crate::{bot_tasks::BotTask, data::report::UserReport, generate, prelude::*};

use futures::future::join_all;
use poise::serenity_prelude::{CacheHttp, ChannelId, MessageId};
use std::sync::Arc;
use tokio::sync::broadcast::{self, error::RecvError};

pub struct SyncTask;

pub struct TaskData<T> {
    pool: sqlx::PgPool,
    reports: broadcast::Receiver<UserReport>,
    cache_http: Arc<T>,
}

impl<T: CacheHttp + 'static> BotTask<T> for SyncTask {
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

async fn sync_menus(data: &TaskData<impl CacheHttp>) -> Result<(), sqlx::Error> {
    let menus = sqlx::query_as::<_, (i64, i64)>(
        "
        SELECT channel_id, message_id
        FROM menu_messages
        ",
    )
    .fetch_all(&data.pool)
    .await?;

    if menus.is_empty() {
        log::debug!("No menu messages to sync, skipping.");
        return Ok(());
    }

    let statuses = generate::menu_status(&data.pool).await?;

    let edit = serenity::EditMessage::default()
        .content(statuses)
        .components(vec![generate::menu_buttons()]);

    let update_all = menus.into_iter().map(|(channel_id, message_id)| {
        let cache_http = Arc::clone(&data.cache_http);
        let channel_id = ChannelId::new(channel_id as u64);
        let message_id = MessageId::new(message_id as u64);
        let edit = edit.clone();

        async move {
            let _ = cache_http
                .http()
                .edit_message(channel_id, message_id, &edit, vec![])
                .await
                .ok();
        }
    });

    join_all(update_all).await;

    Ok(())
}
