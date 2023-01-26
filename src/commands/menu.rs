use crate::{prelude::*, generate, data::menu::{MenuUpdate, MenuId}};

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
        "
    )
    .bind(guild_id.0 as i64)
    .fetch_optional(&mut transaction)
    .await?
    .is_some();

    if menu_exists {
        ctx.say("Menu already exists, use `/menu clear` to delete it.").await?;
        return Ok(());
    }

    let channel_id = ctx.channel_id();

    let statuses = generate::menu_status(&ctx.data().pool).await?;
    let menu_buttons = generate::menu_buttons();

    let menu = channel_id.send_message(ctx, |msg| {
        msg.content(statuses)
            .set_components(menu_buttons)
    }).await?;

    let message_id = menu.id;

    sqlx::query(
        "
        INSERT INTO menu_messages (guild_id, channel_id, message_id)
        VALUES ($1, $2, $3)
        "
    )
    .bind(guild_id.0 as i64)
    .bind(channel_id.0 as i64)
    .bind(message_id.0 as i64)
    .execute(&mut transaction)
    .await?;

    transaction.commit().await?;

    let menu_id = MenuId {
        channel: channel_id,
        message: message_id,
    };

    ctx.data().send_message(MenuUpdate::Create(menu_id, menu));

    ctx.say("Initialized report menu.").await?;

    Ok(())
}

/// (dev-only) Remove the status menu.
#[poise::command(slash_command, ephemeral = true)]
async fn clear(ctx: Context<'_>) -> Result<(), Error> {
    todo!()
}
