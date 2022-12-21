use crate::{data::history_channel::InvalidChannelError, prelude::*};

#[poise::command(slash_command, subcommands("set", "unset"), owners_only)]
pub async fn history(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command, ephemeral = true)]
async fn set(ctx: Context<'_>, channel: serenity::Channel) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let msg = match ctx.data().history_channel.write().await.set(channel) {
        Ok(id) => format!("Successfully set the history to <#{id}>."),
        Err(InvalidChannelError) => String::from("Expected a text-based guild channel."),
    };

    ctx.say(msg).await?;

    Ok(())
}

#[poise::command(slash_command, ephemeral = true)]
async fn unset(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    ctx.data().history_channel.write().await.unset();

    ctx.say("Successfully removed history channel from memory.")
        .await?;

    Ok(())
}
