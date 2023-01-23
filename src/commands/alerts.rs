use itertools::Itertools;

use crate::prelude::*;

#[poise::command(slash_command, subcommands("edit", "list"))]
pub async fn alerts(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Edit your watch list and be alerted when any escalator on it gets reported.
#[poise::command(slash_command, ephemeral = true)]
pub async fn edit(ctx: Context<'_>) -> Result<(), Error> {
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

const ESCALATOR_BUTTON_ID_PREFIX: &str = "ALERTS-ESCALATOR-";
const SUBMIT_BUTTON_ID: &str = "ALERTS-SUBMIT";
const SUBMIT_BUTTON_EMOJI: char = '💾';
