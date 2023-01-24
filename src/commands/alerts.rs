use futures::TryStreamExt;
use indexmap::IndexMap;
use itertools::Itertools;

use crate::prelude::*;

type Watchlist = IndexMap<EscalatorFloors, Subscription>;

struct WatchlistComponent {
    watchlist: Watchlist,
}

enum Subscription {
    Watch,
    Ignore,
}

#[poise::command(slash_command, subcommands("edit", "list"))]
pub async fn alerts(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Edit your watch list and be alerted when any escalator on it gets reported.
#[poise::command(slash_command, ephemeral = true)]
pub async fn edit(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let watchlist = match load_watchlist(&ctx.data().pool, ctx.author().id).await {
        Ok(watchlist) => watchlist,
        Err(err) => {
            log::error!("An error ocurred trying to load watchlist: {err}");
            ctx.say("A database error ocurred.").await?;

            return Ok(());
        }
    };

    let mut watchlist = WatchlistComponent { watchlist };

    todo!()
}

/// Check your watch list
#[poise::command(slash_command, ephemeral = true)]
pub async fn list(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let res = sqlx::query_as::<_, Escalator>(
        "
        SELECT e.floor_start, e.floor_end, e.current_status
        FROM alerts a
        INNER JOIN escalators e
            ON a.floor_start = e.floor_start
            AND a.floor_end = e.floor_end
        WHERE a.user_id = $1
        ORDER BY a.floor_start, a.floor_end
        "
    )
    .bind(ctx.author().id.0 as i64)
    .fetch_all(&ctx.data().pool)
    .await;

    let msg = match res {
        Ok(watchlist) => {
            let body = watchlist
                .into_iter()
                .map(|escalator| {
                    let emoji = escalator.status.emoji();
                    let EscalatorFloors { start, end } = escalator.floors;
                    format!("{emoji} {}-{}", start, end)
                })
                .join("\n");

            format!("**Your Watch List:**```\n{body}```")
        }
        Err(err) => {
            log::error!("An error ocurred generating the watchlist status: {err}");
            String::from("A database error ocurred.")
        }
    };

    ctx.say(msg).await?;

    Ok(())
}

async fn load_watchlist(pool: &sqlx::PgPool, user_id: serenity::UserId) -> Result<Watchlist, sqlx::Error> {
    use sqlx::Row;

    #[derive(sqlx::FromRow)]
    struct WatchlistEntry {
        #[sqlx(flatten)]
        floors: EscalatorFloors,
        #[sqlx(flatten)]
        subscription: Subscription,
    }

    impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for Subscription {
        fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
            let sub = if row.try_get("watching")? {
                Subscription::Watch
            } else {
                Subscription::Ignore
            };

            Ok(sub)
        }
    }

    let mut watchlist = IndexMap::new();

    let mut entries = sqlx::query_as::<_, WatchlistEntry>(
        "
        SELECT e.floor_start, e.floor_end, (a.user_id IS NOT NULL) as watching
        FROM escalators e
        LEFT OUTER JOIN alerts a
            ON e.floor_start = a.floor_start
            AND e.floor_end = a.floor_end
            AND a.user_id = $1
        ORDER BY e.floor_start + e.floor_end, e.floor_start
        "
    )
    .bind(user_id.0 as i64)
    .fetch(pool);

    while let Some(entry) = entries.try_next().await? {
        watchlist.insert(entry.floors, entry.subscription);
    }

    Ok(watchlist)
}

const ESCALATOR_BUTTON_ID_PREFIX: &str = "ALERTS-ESCALATOR-";
const SUBMIT_BUTTON_ID: &str = "ALERTS-SUBMIT";
const SUBMIT_BUTTON_EMOJI: char = '💾';
