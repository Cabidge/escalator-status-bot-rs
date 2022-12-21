mod history;
mod menu;

use crate::prelude::*;

/// Returns a vector containing all enabled bot commands.
pub fn commands() -> Vec<poise::Command<crate::Data, Error>> {
    vec![
        register(),
        ping(),
        menu::menu(),
        history::history(),
        gist(),
        kill(),
    ]
}

/// Spawns a button panel to register application commands (dev-only).
#[poise::command(prefix_command, owners_only)]
async fn register(ctx: Context<'_>) -> Result<(), Error> {
    poise::builtins::register_application_commands_buttons(ctx).await?;
    Ok(())
}

/// Kills the bot (dev-only).
#[poise::command(prefix_command, owners_only)]
async fn kill(ctx: Context<'_>) -> Result<(), Error> {
    ctx.data().shard_manager.lock().await.shutdown_all().await;
    Ok(())
}

/// Pings the bot for it to pong back.
#[poise::command(slash_command, ephemeral = true)]
async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("Pong!").await?;
    Ok(())
}

#[poise::command(slash_command, ephemeral = true)]
async fn gist(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let gist = ctx.data().statuses.lock().await.gist();
    ctx.send(move |msg| msg.embed(replace_builder_with(gist)))
        .await?;

    Ok(())
}
