use crate::prelude::*;

#[poise::command(slash_command, subcommands("init", "clear"), owners_only)]
pub async fn menu(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command, ephemeral = true)]
async fn init(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    ctx.data().report_menu.lock().await.initialize(ctx).await?;

    Ok(())
}

#[poise::command(slash_command, ephemeral = true)]
async fn clear(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    ctx.data().report_menu.lock().await.clear(ctx).await?;

    Ok(())
}
