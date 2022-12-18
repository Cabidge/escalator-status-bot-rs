use crate::{data::status::Status, prelude::*};

pub use poise::serenity_prelude as serenity;

const REPORT_OPEN_ID: &str = "BTN_REPORT_OPEN";
const REPORT_DOWN_ID: &str = "BTN_REPORT_DOWN";
const REPORT_BLOCKED_ID: &str = "BTN_REPORT_BLOCKED";

/// Handles an interaction created by a user.
pub async fn handle_interaction(
    _http: impl AsRef<serenity::Http>,
    _interaction: &serenity::Interaction,
    _data: &crate::Data,
) -> Result<(), Error> {
    Ok(())
}

/// Generate report button components with the proper IDs.
pub fn report_buttons() -> serenity::CreateComponents {
    let mut components = serenity::CreateComponents::default();

    // TODO: DRY, this is unecessarily verbose
    components.create_action_row(|row| {
        row.create_button(|btn| {
            btn.label("Report Open")
                .emoji(Status::Open.emoji())
                .style(serenity::ButtonStyle::Success)
                .custom_id(REPORT_OPEN_ID)
        })
        .create_button(|btn| {
            btn.label("Report Down")
                .emoji(Status::Down.emoji())
                .style(serenity::ButtonStyle::Danger)
                .custom_id(REPORT_DOWN_ID)
        })
        .create_button(|btn| {
            btn.label("Report Blocked")
                .emoji(Status::Blocked.emoji())
                .style(serenity::ButtonStyle::Danger)
                .custom_id(REPORT_BLOCKED_ID)
        })
    });

    components
}
