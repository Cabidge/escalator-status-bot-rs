use std::time::Duration;

use crate::{
    data::{status::Status, EscalatorInput},
    prelude::*,
    report_modal::ReportModal,
};

pub use poise::serenity_prelude as serenity;
use poise::Modal;

const REPORT_OPEN_ID: &str = "BTN_REPORT_OPEN";
const REPORT_DOWN_ID: &str = "BTN_REPORT_DOWN";
const REPORT_BLOCKED_ID: &str = "BTN_REPORT_BLOCKED";

/// Handles an interaction created by a user.
pub async fn handle_interaction(
    serenity_ctx: &serenity::Context,
    interaction: &serenity::Interaction,
    data: &Data,
) -> Result<(), Error> {
    let Some(interaction) = interaction.to_owned().message_component() else { return Ok(()) };

    let status = match interaction.data.custom_id.as_str() {
        REPORT_OPEN_ID => Status::Open,
        REPORT_DOWN_ID => Status::Down,
        REPORT_BLOCKED_ID => Status::Blocked,
        _ => return Ok(()),
    };

    // generate modal id from the interaction id
    let modal_id = format!("REPORT-MODAL-{}", interaction.id);

    // override interaction response with the modal
    let modal = ReportModal::create(None, modal_id.clone());
    interaction
        .create_interaction_response(serenity_ctx, replace_builder_with(modal))
        .await?;

    let Some(response) = serenity::CollectModalInteraction::new(&serenity_ctx.shard)
        .filter(move |modal_interaction| modal_interaction.data.custom_id == modal_id)
        .timeout(Duration::from_secs(60 * 60))
        .await
        else { return Ok(())};

    response
        .create_interaction_response(serenity_ctx, |res| {
            res.kind(serenity::InteractionResponseType::DeferredUpdateMessage)
        })
        .await?;

    let modal_information =
        ReportModal::parse(response.data.clone()).map_err(serenity::Error::Other)?;

    let escalators = match EscalatorInput::try_from(modal_information) {
        Ok(e) => e,
        Err(err) => {
            interaction
                .create_followup_message(serenity_ctx, |msg| {
                    msg.content(err.to_string()).ephemeral(true)
                })
                .await?;

            return Ok(());
        }
    };

    data.statuses.lock().await.report(escalators, status);

    let message = format!(
        "You've successfully reported {} as `{}`",
        escalators.message_noun(),
        format!("{status:?}").to_uppercase()
    );

    interaction
        .create_followup_message(serenity_ctx, |msg| msg.content(message).ephemeral(true))
        .await?;

    Ok(())
}

/// Generate report button components with the proper IDs.
pub fn add_report_buttons(
    components: &mut serenity::CreateComponents,
) -> &mut serenity::CreateComponents {
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
    })
}
