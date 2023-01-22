use crate::{prelude::*, data::status::Status};

use indexmap::IndexMap;
use serde::{Serialize, Deserialize};

const ESCALATOR_COUNT: usize = 14;

const ESCALATORS: [Escalator; ESCALATOR_COUNT] = [
    (2, 3), // 0
    (2, 4), // 1
    (3, 2), // 2
    (3, 5), // 3
    (4, 2), // 4
    (4, 6), // 5
    (5, 3), // 6
    (5, 7), // 7
    (6, 4), // 8
    (6, 8), // 9
    (7, 5), // 10
    (7, 9), // 11
    (8, 6), // 12
    (9, 7), // 13
];

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct Info {
    last_update: std::time::SystemTime,
    status: Option<Status>,
}

type Escalators = IndexMap<Escalator, Info>;

type WatchLists = Vec<(u64, [bool; ESCALATOR_COUNT])>;

pub async fn migrate_to_sqlx(persist: &shuttle_persist::PersistInstance, pool: &sqlx::PgPool, ctx: &serenity::Context) {
    if persist.load::<bool>("migrated").unwrap_or_default() {
        log::debug!("Already migrated to sql database, skipping...");
        return;
    }

    log::info!("Persist not migrated yet, migrating...");

    // escalator statuses
    if let Ok(escalators) = persist.load::<Escalators>("escalators") {
        log::info!("Migrating statuses...");
        for ((start, end), info) in escalators {
            let status = info.status.unwrap_or(Status::Open);

            if status == Status::Open {
                continue;
            }

            let res = sqlx::query(
                "
                UPDATE escalators
                SET current_status = $1
                WHERE floor_Start = $2
                AND floor_end = $3
                "
            )
            .bind(status)
            .bind(start as i16)
            .bind(end as i16)
            .execute(pool)
            .await;

            if let Err(err) = res {
                log::warn!("Error migrating statuses: {}", err);
            }
        }
    }

    // alerts/watchlists
    if let Ok(watchlists) = persist.load::<WatchLists>("alerts") {
        let values = watchlists.into_iter()
            .flat_map(|(user_id, watchlist)| {
                watchlist.into_iter()
                    .zip(ESCALATORS)
                    .filter_map(move |(active, escalator)| active.then_some((user_id, escalator)))
            });

        let res = sqlx::QueryBuilder::new("INSERT INTO alerts (user_id, floor_start, floor_end) ")
            .push_values(values, |mut value, (user_id, (start, end))| {
                value.push_bind(user_id as i64)
                    .push_bind(start as i16)
                    .push_bind(end as i16);
            })
            .build()
            .execute(pool)
            .await;

        if let Err(err) = res {
            log::warn!("Error migrating alerts: {}", err);
        }
    }

    // history/announcements channel
    (async {
        let channel_id = persist.load::<Option<u64>>("history_channel").ok().flatten()?;

        let channel = serenity::ChannelId(channel_id)
            .to_channel(&ctx.http)
            .await
            .ok()?
            .guild()?;

        let guild_id = channel.guild_id.0;

        let res = sqlx::query(
            "
            INSERT INTO announcement_channels (guild_id, channel_id)
            VALUES ($1, $2)
            "
        )
        .bind(guild_id as i64)
        .bind(channel_id as i64)
        .execute(pool)
        .await;

        if let Err(err) = res {
            log::warn!("Error migrating history channel: {}", err);
        }

        Some(())
    }).await;

    // report menu message
    (async {
        let (channel_id, message_id) = persist.load::<Option<(u64, u64)>>("report_menu").ok().flatten()?;

        let channel = serenity::ChannelId(channel_id)
            .to_channel(&ctx.http)
            .await
            .ok()?
            .guild()?;

        let guild_id = channel.guild_id.0;

        let res = sqlx::query(
            "
            INSERT INTO menu_messages (guild_id, channel_id, message_id)
            VALUES ($1, $2, $3)
            "
        )
        .bind(guild_id as i64)
        .bind(channel_id as i64)
        .bind(message_id as i64)
        .execute(pool)
        .await;

        if let Err(err) = res {
            log::warn!("Error migrating report menu message channel: {}", err);
        }

        Some(())
    }).await;

    if let Err(err) = persist.save("migrated", true) {
        log::error!("Error setting migrated to true {}", err);
    }

    log::info!("Finished migrations!");
}