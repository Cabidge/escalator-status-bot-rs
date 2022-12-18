use crate::{data::status::Status, prelude::*, report_modal::ReportModal};

use poise::Modal;
pub use poise::serenity_prelude as serenity;

const REPORT_OPEN_ID: &str = "BTN_REPORT_OPEN";
const REPORT_DOWN_ID: &str = "BTN_REPORT_DOWN";
const REPORT_BLOCKED_ID: &str = "BTN_REPORT_BLOCKED";

/// Handles an interaction created by a user.
pub async fn handle_interaction(
    http: impl AsRef<serenity::Http>,
    interaction: &serenity::Interaction,
    _data: &crate::Data,
) -> Result<(), Error> {
    let Some(interaction) = interaction.to_owned().message_component() else { return Ok(()) };
    let http = http.as_ref();

    let _status = match interaction.data.custom_id.as_str() {
        REPORT_OPEN_ID => Status::Open,
        REPORT_DOWN_ID => Status::Down,
        REPORT_BLOCKED_ID => Status::Blocked,
        _ => return Ok(()),
    };

    interaction.create_followup_message(http, |msg| {
        msg.content("Test")
            .ephemeral(true)
    }).await?;

    let modal_id = format!("REPORT-MODAL-{}", interaction.id);

    // override interaction response with the modal
    let modal = ReportModal::create(None, modal_id);
    interaction.create_interaction_response(http, |res| {
        *res = modal;
        res
    }).await?;

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
