use itertools::Itertools;

use crate::prelude::*;

#[poise::command(slash_command, subcommands("add", "remove", "list"))]
pub async fn alerts(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Add an escalator to your watch list and be alerted when it gets reported
#[poise::command(slash_command, ephemeral = true)]
pub async fn add(
    ctx: Context<'_>,
    #[description = "The beginning floor of the escalator"] start: u8,
    #[description = "The ending floor of the escalator"] end: u8,
    #[description = "Add both the up and down escalator"] pair: Option<bool>,
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    if !is_valid_escalator(start, end) {
        ctx.say(format!("The `{}-{}` is not a valid escalator.", start, end)).await?;
        return Ok(());
    }
    
    let mut alerts = ctx.data().alerts.lock().await;

    alerts.add(ctx.author(), (start, end));

    // add the inverse escalator if pair is true
    if pair == Some(true) {
        alerts.add(ctx.author(), (end, start));
    }

    ctx.say("Watch list updated.").await?;

    Ok(())
}

/// Remove an escalator from your watch list to no longer be alerted when it gets reported
#[poise::command(slash_command, ephemeral = true)]
pub async fn remove(
    ctx: Context<'_>,
    #[description = "The beginning floor of the escalator"] start: u8,
    #[description = "The ending floor of the escalator"] end: u8,
    #[description = "Add both the up and down escalator"] pair: Option<bool>,
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    if !is_valid_escalator(start, end) {
        ctx.say(format!("The `{}-{}` is not a valid escalator.", start, end)).await?;
        return Ok(());
    }

    let mut alerts = ctx.data().alerts.lock().await;

    alerts.remove(ctx.author(), (start, end));

    // remove the inverse escalator if pair is true
    if pair == Some(true) {
        alerts.remove(ctx.author(), (end, start));
    }

    ctx.say("Watch list updated.").await?;

    Ok(())
}

/// Check your watch list
#[poise::command(slash_command, ephemeral = true)]
pub async fn list(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let alerts = ctx.data().alerts.lock().await;

    let Some(watch_list) = alerts.get_watch_list(ctx.author()) else {
        ctx.say("Your watch list is empty. Try adding escalators to it with the `/alert add <start> <end>` command.").await?;
        return Ok(());
    };

    let statuses = ctx.data().statuses.lock().await;
    let message = String::from("**Your Watch List:**```\n")
        + &watch_list
            .iter()
            .sorted()
            .filter_map(|escalator| {
                let info = statuses.get_info(*escalator)?;
                Some(format!("{} {}-{}", info.status_emoji(), escalator.0, escalator.1))
            })
            .join("\n")
        + "```";

    ctx.say(message).await?;

    Ok(())
}
