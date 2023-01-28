use std::{str::FromStr, time::Duration};

use futures::{StreamExt, TryStreamExt};
use indexmap::IndexMap;
use itertools::Itertools;

use crate::{generate, prelude::*};

type Watchlist = IndexMap<EscalatorFloors, Subscription>;

struct WatchlistComponent {
    watchlist: Watchlist,
}

#[derive(Debug, Clone, Copy)]
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
    const TIMEOUT: Duration = Duration::from_secs(2 * 60);

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

    let handle = ctx
        .send(|msg| {
            msg.content(generate::timeout_message(TIMEOUT))
                .components(replace_builder_with(watchlist.render()))
        })
        .await?;

    let mut actions = handle
        .message()
        .await?
        .await_component_interactions(&ctx.serenity_context().shard)
        .build();

    let res = loop {
        let sleep = tokio::time::sleep(TIMEOUT);
        tokio::pin!(sleep);

        let action = tokio::select! {
            Some(action) = actions.next() => action,
            _ = sleep => break None,
        };

        action.defer(ctx).await?;

        let command = match action.data.custom_id.parse::<ComponentAction>() {
            Ok(command) => command,
            Err(err) => {
                log::warn!("An error ocurred parsing a component command: {err}");
                continue;
            }
        };

        if let ComponentStatus::Complete(watchlist) = watchlist.execute(command) {
            break Some(watchlist);
        }

        handle
            .edit(ctx, |msg| {
                msg.content(generate::timeout_message(TIMEOUT))
                    .components(replace_builder_with(watchlist.render()))
            })
            .await?;
    };

    actions.stop();

    // clear the components
    handle
        .edit(ctx, |msg| {
            msg.content("Processing...")
                .components(|components| components.set_action_rows(vec![]))
        })
        .await?;

    let Some(watchlist) = res else {
        handle.edit(ctx, |msg| {
            msg.content("Interaction timed out, try again...")
        }).await?;

        return Ok(());
    };

    if let Err(err) = update_watchlist(&ctx.data().pool, ctx.author().id, watchlist).await {
        log::error!("An error ocurred trying to update watchlist: {err}");
        handle
            .edit(ctx, |msg| msg.content("A database error ocurred."))
            .await?;

        return Ok(());
    }

    handle
        .edit(ctx, |msg| msg.content("Watchlist updated."))
        .await?;

    Ok(())
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
        ",
    )
    .bind(ctx.author().id.0 as i64)
    .fetch_all(&ctx.data().pool)
    .await;

    let msg = match res {
        Ok(watchlist) => {
            let body = watchlist.iter().map(Escalator::to_string).join("\n");

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

async fn load_watchlist(
    pool: &sqlx::PgPool,
    user_id: serenity::UserId,
) -> Result<Watchlist, sqlx::Error> {
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
        ",
    )
    .bind(user_id.0 as i64)
    .fetch(pool);

    while let Some(entry) = entries.try_next().await? {
        watchlist.insert(entry.floors, entry.subscription);
    }

    Ok(watchlist)
}

async fn update_watchlist(
    pool: &sqlx::PgPool,
    user_id: serenity::UserId,
    watchlist: &Watchlist,
) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin().await?;

    let mut starts = vec![];
    let mut ends = vec![];

    let watching = watchlist
        .iter()
        .filter_map(|(floors, sub)| (sub.is_watching()).then_some(*floors));

    for EscalatorFloors { start, end } in watching {
        starts.push(start as i16);
        ends.push(end as i16);
    }

    sqlx::query(
        "
        DELETE FROM alerts a
        WHERE user_id = $1
        AND NOT EXISTS (
            SELECT FROM UNNEST($2::smallint[], $3::smallint[])
                AS w (floor_start, floor_end)
            WHERE a.floor_start = w.floor_start
            AND a.floor_end = w.floor_end
        )
        ",
    )
    .bind(user_id.0 as i64)
    .bind(&starts)
    .bind(&ends)
    .execute(&mut transaction)
    .await?;

    sqlx::query(
        "
        INSERT INTO alerts (user_id, floor_start, floor_end)
        SELECT $1 as user_id, w.floor_start, w.floor_end
        FROM UNNEST($2::smallint[], $3::smallint[])
            AS w (floor_start, floor_end)
        LEFT OUTER JOIN alerts a
            ON $1 = a.user_id
            AND w.floor_start = a.floor_start
            AND w.floor_end = a.floor_end
        WHERE a.user_id IS NULL
        ",
    )
    .bind(user_id.0 as i64)
    .bind(&starts)
    .bind(&ends)
    .execute(&mut transaction)
    .await?;

    transaction.commit().await
}

const ESCALATOR_BUTTON_ID_PREFIX: &str = "ALERTS-ESCALATOR-";
const SUBMIT_BUTTON_ID: &str = "ALERTS-SUBMIT";
const SUBMIT_BUTTON_EMOJI: char = '💾';

enum ComponentStatus<T> {
    Continue,
    Complete(T),
}

enum ComponentAction {
    Toggle(EscalatorFloors),
    Submit,
}

impl WatchlistComponent {
    fn render(&self) -> serenity::CreateComponents {
        let mut action_rows = self
            .watchlist
            .iter()
            .chunks(4)
            .into_iter()
            .map(|row| {
                let mut action_row = serenity::CreateActionRow::default();

                for (&floors, &sub) in row {
                    let id = format!("{ESCALATOR_BUTTON_ID_PREFIX}{floors}");

                    let style = match sub {
                        Subscription::Watch => serenity::ButtonStyle::Primary,
                        Subscription::Ignore => serenity::ButtonStyle::Secondary,
                    };

                    action_row
                        .create_button(|button| button.label(floors).custom_id(id).style(style));
                }

                action_row
            })
            .collect_vec();

        action_rows.last_mut().unwrap().create_button(|button| {
            button
                .label("Save List")
                .custom_id(SUBMIT_BUTTON_ID)
                .style(serenity::ButtonStyle::Success)
                .emoji(SUBMIT_BUTTON_EMOJI)
        });

        let mut components = serenity::CreateComponents::default();

        components.set_action_rows(action_rows);

        components
    }

    fn execute(&mut self, command: ComponentAction) -> ComponentStatus<&Watchlist> {
        match command {
            ComponentAction::Submit => ComponentStatus::Complete(&self.watchlist),
            ComponentAction::Toggle(floors) => {
                if let Some(sub) = self.watchlist.get_mut(&floors) {
                    sub.toggle();
                }

                ComponentStatus::Continue
            }
        }
    }
}

impl Subscription {
    fn toggle(&mut self) {
        match self {
            Self::Watch => *self = Self::Ignore,
            Self::Ignore => *self = Self::Watch,
        }
    }

    fn is_watching(&self) -> bool {
        match self {
            Self::Watch => true,
            Self::Ignore => false,
        }
    }
}

impl FromStr for ComponentAction {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(floors) = s.strip_prefix(ESCALATOR_BUTTON_ID_PREFIX) {
            let (start, end) = floors
                .split_once('-')
                .ok_or_else(|| anyhow::anyhow!("Invalid escalator"))?;

            let start = start.parse::<u8>()?;
            let end = end.parse::<u8>()?;

            return Ok(ComponentAction::Toggle(EscalatorFloors { start, end }));
        }

        if s == SUBMIT_BUTTON_ID {
            return Ok(ComponentAction::Submit);
        }

        anyhow::bail!("Unknown command");
    }
}
