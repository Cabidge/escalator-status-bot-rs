mod alerts;
mod history;
mod menu;

use crate::{generate, prelude::*};

/// Returns a vector containing all enabled bot commands.
pub fn commands() -> Vec<poise::Command<crate::Data, Error>> {
    vec![
        register(),
        kill(),
        menu::menu(),
        history::history(),
        alerts::alerts(),
        gist(),
    ]
}

/// (dev-only) Spawn a button panel to register application commands.
#[poise::command(prefix_command, owners_only)]
async fn register(ctx: Context<'_>) -> Result<(), Error> {
    poise::builtins::register_application_commands_buttons(ctx).await?;
    Ok(())
}

/// (dev-only) Force quit the bot.
#[poise::command(prefix_command, owners_only)]
async fn kill(ctx: Context<'_>) -> Result<(), Error> {
    ctx.data().shard_manager.lock().await.shutdown_all().await;
    Ok(())
}

/// Display a summary of the escalator statuses.
#[poise::command(slash_command, ephemeral = true)]
async fn gist(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    match generate::gist(&ctx.data().pool).await {
        Ok(gist) => {
            ctx.send(move |msg| msg.embed(replace_builder_with(gist)))
                .await?;
        }
        Err(err) => {
            log::error!("An error ocurred trying to generate a gist: {err}");
            ctx.say("A database error ocurred.").await?;
        }
    }

    Ok(())
}
