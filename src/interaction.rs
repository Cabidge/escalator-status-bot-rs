use crate::prelude::*;

pub use poise::serenity_prelude as serenity;

/// Handles an interaction created by a user.
pub async fn handle_interaction(
    _http: impl AsRef<serenity::Http>,
    _interaction: &serenity::Interaction,
    _data: &crate::Data,
) -> Result<(), Error> {
    Ok(())
}
