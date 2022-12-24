use std::time::Duration;

use anyhow::anyhow;
use itertools::Itertools;
use poise::futures_util::StreamExt;

use crate::{
    data::{ESCALATORS, ESCALATOR_COUNT, PAIR_ORDER},
    prelude::*,
};

#[poise::command(slash_command, subcommands("edit", "list"))]
pub async fn alerts(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

struct WatchListComponent {
    watch_list: [bool; ESCALATOR_COUNT],
}

/// Edit your watch list and be alerted when any escalator on it gets reported.
#[poise::command(slash_command, ephemeral = true)]
pub async fn edit(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let watch_list = ctx
        .data()
        .alerts
        .lock()
        .await
        .get_watch_list(ctx.author())
        .unwrap_or_default(); // jesus why is this so long

    let mut components = WatchListComponent { watch_list };

    let handle = ctx
        .send(|msg| msg.components(replace_builder_with(components.render())))
        .await?;

    let mut actions = handle
        .message()
        .await?
        .await_component_interactions(&ctx.serenity_context().shard)
        .build();

    let res = loop {
        let sleep = tokio::time::sleep(Duration::from_secs(2 * 60));
        tokio::pin!(sleep);

        let action = tokio::select! {
            Some(action) = actions.next() => action,
            _ = sleep => break Err(anyhow!("Timeout")),
        };

        action.defer(ctx).await?;

        if let Some(list) = components.try_action(&action.data.custom_id) {
            break Ok(list);
        }

        handle
            .edit(ctx, |msg| {
                msg.components(replace_builder_with(components.render()))
            })
            .await?;
    };

    actions.stop();

    let Ok(watch_list) = res else {
        handle.edit(ctx, |msg| {
            msg.content("Interaction timed out, try again...")
                .components(|components| {
                    components.set_action_rows(vec![])
                })
        }).await?;
        return Ok(());
    };

    ctx.data()
        .alerts
        .lock()
        .await
        .replace(ctx.author(), watch_list);

    handle
        .edit(ctx, |msg| {
            msg.content("Watch list updated.")
                .components(|components| components.set_action_rows(vec![]))
        })
        .await?;

    Ok(())
}

/// Check your watch list
#[poise::command(slash_command, ephemeral = true)]
pub async fn list(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let alerts = ctx.data().alerts.lock().await;

    let Some(watch_list) = alerts.get_watch_list(ctx.author()) else {
        ctx.say("Your watch list is empty. Try adding escalators to it with the `/alerts edit` command.").await?;
        return Ok(());
    };

    let statuses = ctx.data().statuses.lock().await;
    let message = String::from("**Your Watch List:**```\n")
        + &watch_list
            .into_iter()
            .enumerate()
            .filter_map(|(index, selected)| {
                if !selected {
                    return None;
                };
                let escalator = ESCALATORS[index];
                let emoji = statuses.get_info(escalator)?.status_emoji();
                Some(format_escalator(emoji, escalator))
            })
            .join("\n")
        + "```";

    ctx.say(message).await?;

    Ok(())
}

const ESCALATOR_BUTTON_ID_PREFIX: &str = "ALERTS-ESCALATOR-";
const SUBMIT_BUTTON_ID: &str = "ALERTS-SUBMIT";
const SUBMIT_BUTTON_EMOJI: char = '💾';

impl WatchListComponent {
    fn try_action(&mut self, action_id: &str) -> Option<[bool; ESCALATOR_COUNT]> {
        if let Some(index) = action_id.strip_prefix(ESCALATOR_BUTTON_ID_PREFIX) {
            self.toggle_index(index.parse().ok()?);
            return None;
        }

        if action_id == SUBMIT_BUTTON_ID {
            return Some(self.watch_list);
        }

        None
    }

    fn toggle_index(&mut self, index: usize) {
        if index < ESCALATOR_COUNT {
            self.watch_list[index] = !self.watch_list[index];
        }
    }

    fn render(&self) -> serenity::CreateComponents {
        let mut action_rows = PAIR_ORDER
            .chunks(4)
            .map(|row| {
                let mut action_row = serenity::CreateActionRow::default();

                for &index in row {
                    action_row.create_button(|button| self.escalator_button(button, index));
                }

                action_row
            })
            .collect_vec();

        action_rows.last_mut().unwrap().create_button(|button| {
            button
                .label("Save List")
                .custom_id(SUBMIT_BUTTON_ID)
                .style(serenity::ButtonStyle::Success)
                .emoji(SUBMIT_BUTTON_EMOJI)
        });

        let mut components = serenity::CreateComponents::default();

        components.set_action_rows(action_rows);

        components
    }

    fn escalator_button<'a>(
        &self,
        button: &'a mut serenity::CreateButton,
        index: usize,
    ) -> &'a mut serenity::CreateButton {
        let (start, end) = ESCALATORS[index];
        let label = format!("{}-{}", start, end);

        let id = format!("{}{}", ESCALATOR_BUTTON_ID_PREFIX, index);

        let style = if self.watch_list[index] {
            serenity::ButtonStyle::Primary
        } else {
            serenity::ButtonStyle::Secondary
        };

        button.label(label).custom_id(id).style(style)
    }
}

fn format_escalator(emoji: char, (start, end): Escalator) -> String {
    format!("{} {}-{}", emoji, start, end)
}
