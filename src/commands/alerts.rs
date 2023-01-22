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
    todo!();
}

const ESCALATOR_BUTTON_ID_PREFIX: &str = "ALERTS-ESCALATOR-";
const SUBMIT_BUTTON_ID: &str = "ALERTS-SUBMIT";
const SUBMIT_BUTTON_EMOJI: char = '💾';
