use crate::prelude::*;

#[poise::command(slash_command, subcommands("set", "remove"), owners_only)]
pub async fn history(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// (dev-only) Set the announcements channel for this server.
#[poise::command(slash_command, ephemeral = true)]
async fn set(ctx: Context<'_>, channel: serenity::Channel) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let Some(guild_id) = ctx.guild_id() else {
        ctx.say("This command must be used in a server.").await?;
        return Ok(());
    };

    let res = sqlx::query(
        "
        INSERT INTO announcement_channels (guild_id, channel_id)
        VALUES ($1, $2)
        ON CONFLICT (guild_id)
            DO UPDATE SET channel_id = $2
        ",
    )
    .bind(guild_id.0 as i64)
    .bind(channel.id().0 as i64)
    .execute(&ctx.data().pool)
    .await;

    let msg = match res {
        Ok(_) => format!("Set history channel to <#{}>.", channel.id()),
        Err(err) => {
            log::warn!("An error ocurred while updating the history channel: {err}");
            String::from("A database error ocurred.")
        }
    };

    ctx.say(msg).await?;

    Ok(())
}

/// (dev-only) Remove the announcements channel for this server.
#[poise::command(slash_command, ephemeral = true)]
async fn remove(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let Some(guild_id) = ctx.guild_id() else {
        ctx.say("This command must be used in a server.").await?;
        return Ok(());
    };

    let res = sqlx::query(
        "
        DELETE FROM announcement_channels
        WHERE guild_id = $1
        ",
    )
    .bind(guild_id.0 as i64)
    .execute(&ctx.data().pool)
    .await;

    let msg = match res {
        Ok(_) => format!("Removed history channel"),
        Err(err) => {
            log::warn!("An error ocurred while removing the history channel: {err}");
            String::from("A database error ocurred.")
        }
    };

    ctx.say(msg).await?;

    Ok(())
}
