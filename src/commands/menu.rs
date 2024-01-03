use crate::{generate, prelude::*};

use poise::serenity_prelude::CacheHttp;

#[poise::command(slash_command, subcommands("init", "clear"), owners_only)]
pub async fn menu(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// (dev-only) Initialize the status menu in the current channel.
#[poise::command(slash_command, ephemeral = true)]
async fn init(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let Some(guild_id) = ctx.guild_id() else {
        ctx.say("`/menu init` must be used in a guild.").await?;
        return Ok(());
    };

    let mut transaction = ctx.data().pool.begin().await?;

    let menu_exists = sqlx::query(
        "
        SELECT 1
        FROM menu_messages
        WHERE guild_id = $1
        ",
    )
    .bind(guild_id.0 as i64)
    .fetch_optional(&mut *transaction)
    .await?
    .is_some();

    if menu_exists {
        ctx.say("Menu already exists, use `/menu clear` to delete it.")
            .await?;
        return Ok(());
    }

    let channel_id = ctx.channel_id();

    let statuses = generate::menu_status(&ctx.data().pool).await?;
    let menu_buttons = generate::menu_buttons();

    let menu = channel_id
        .send_message(ctx, |msg| {
            msg.content(statuses).set_components(menu_buttons)
        })
        .await?;

    let message_id = menu.id;

    sqlx::query(
        "
        INSERT INTO menu_messages (guild_id, channel_id, message_id)
        VALUES ($1, $2, $3)
        ",
    )
    .bind(guild_id.0 as i64)
    .bind(channel_id.0 as i64)
    .bind(message_id.0 as i64)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    ctx.say("Initialized report menu.").await?;

    Ok(())
}

/// (dev-only) Remove the status menu.
#[poise::command(slash_command, ephemeral = true)]
async fn clear(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let Some(guild_id) = ctx.guild_id() else {
        ctx.say("`/menu clear` must be used in a guild.").await?;
        return Ok(());
    };

    let Some((channel_id, message_id)) = sqlx::query_as::<_, (i64, i64)>(
        "
        DELETE FROM menu_messages
        WHERE guild_id = $1
        RETURNING channel_id, message_id
        "
    )
    .bind(guild_id.0 as i64)
    .fetch_optional(&ctx.data().pool)
    .await? else {
        ctx.say("No report menu exists in this server.").await?;
        return Ok(());
    };

    let res = ctx
        .http()
        .delete_message(channel_id as u64, message_id as u64)
        .await;

    if let Err(err) = res {
        log::warn!("An error ocurred trying to delete report menu: {err}");
    }

    ctx.say("Deleted report menu").await?;

    Ok(())
}
