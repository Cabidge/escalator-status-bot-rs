use std::{sync::Arc, time::Duration};

use crate::{prelude::*, report_modal::ReportModal};

use super::{escalator_input::EscalatorInput, status::Status, Statuses, UserReport};

use poise::{futures_util::StreamExt, Modal};
use shuttle_persist::PersistInstance;
use tokio::{sync::mpsc, task};

#[derive(Debug)]
pub struct ReportMenu {
    shard_messenger: serenity::ShardMessenger,
    user_reports: mpsc::Sender<UserReport>,
    menu: Option<MenuHandle>,
    should_save: bool,
}

#[derive(Debug)]
struct MenuHandle {
    message: serenity::Message,
    interaction_task: task::JoinHandle<()>,
}

impl ReportMenu {
    pub async fn load_persist(
        user_reports: mpsc::Sender<UserReport>,
        ctx: &serenity::Context,
        persist: &PersistInstance,
    ) -> Self {
        let Ok(ids) = persist.load::<Option<(u64, u64)>>("report_menu") else {
            return Self::new(ctx.shard.clone(), user_reports, true);
        };

        let Some(ids) = ids else {
            return Self::new(ctx.shard.clone(), user_reports, false);
        };

        let channel_id = serenity::ChannelId(ids.0);
        let message_id = serenity::MessageId(ids.1);

        let Ok(message) = channel_id.message(ctx, message_id).await else {
            return Self::new(ctx.shard.clone(), user_reports, true);
        };

        let mut report_menu = Self::new(ctx.shard.clone(), user_reports, false);
        let menu = report_menu.create_handle(Arc::clone(&ctx.http), message);

        report_menu.menu = Some(menu);

        report_menu
    }

    pub fn save_persist(&mut self, persist: &PersistInstance) {
        if self.should_save {
            let ids = self.menu.as_ref().map(MenuHandle::ids);

            let _ = persist.save("report_menu", ids).ok();

            self.should_save = false;
        }
    }

    fn new(
        shard_messenger: serenity::ShardMessenger,
        user_reports: mpsc::Sender<UserReport>,
        should_save: bool,
    ) -> Self {
        Self {
            menu: None,
            shard_messenger,
            user_reports,
            should_save,
        }
    }

    pub fn is_initialized(&self) -> bool {
        self.menu.is_some()
    }

    /// Tries to update the report menu message.
    ///
    /// Only returns an error if there was an error trying to edit the message.
    pub async fn update(
        &mut self,
        cache_http: impl serenity::CacheHttp,
        statuses: &Statuses,
    ) -> Result<(), Error> {
        let Some(menu) = &mut self.menu else { return Ok(()) };

        let content = statuses.menu_message();
        menu.update_message(cache_http, &content).await?;

        Ok(())
    }

    pub async fn initialize(&mut self, ctx: Context<'_>) -> Result<(), Error> {
        if self.is_initialized() {
            ctx.send(|msg| {
                msg.content("A report menu already exists, run `/menu clear` to delete it and clear it from memory.")
                    .ephemeral(true)
            }).await?;

            return Ok(());
        }

        let content = ctx.data().statuses.lock().await.menu_message();

        let handle = ctx
            .send(|msg| {
                msg.content(content)
                    .components(add_report_buttons)
                    .ephemeral(false)
            })
            .await?;

        let message = handle.into_message().await?;
        let http = Arc::clone(&ctx.serenity_context().http);

        let menu = self.create_handle(http, message);
        self.menu = Some(menu);
        self.should_save = true;

        Ok(())
    }

    pub async fn clear(&mut self, ctx: Context<'_>) -> Result<(), Error> {
        let Some(menu) = self.menu.take() else {
            ctx.say("Report menu is already cleared.").await?;
            return Ok(());
        };

        self.should_save = true;

        menu.delete(ctx).await;

        ctx.say("Report menu successfully cleared from memory.")
            .await?;

        Ok(())
    }

    fn create_handle(&self, http: Arc<serenity::Http>, message: serenity::Message) -> MenuHandle {
        let mut collector = message
            .await_component_interactions(&self.shard_messenger)
            .build();
        let user_reports = self.user_reports.downgrade();
        let shard = self.shard_messenger.clone();

        let interaction_task = tokio::spawn(async move {
            while let Some(interaction) = collector.next().await {
                let Some(user_reports) = user_reports.upgrade() else {
                    break;
                };

                // TODO: log error
                if let Err(err) =
                    handle_interaction(&interaction, user_reports, &http, &shard).await
                {
                    println!("{err:?}");
                }
            }
        });

        MenuHandle {
            message,
            interaction_task,
        }
    }
}

impl MenuHandle {
    async fn delete(self, cache_http: impl serenity::CacheHttp) {
        self.interaction_task.abort();
        let _ = self.message.delete(cache_http).await.ok();
    }

    async fn update_message(
        &mut self,
        cache_http: impl serenity::CacheHttp,
        content: &str,
    ) -> Result<(), Error> {
        self.message
            .edit(cache_http, |msg| msg.content(content))
            .await?;

        Ok(())
    }

    fn ids(&self) -> (u64, u64) {
        (self.message.channel_id.0, self.message.id.0)
    }
}

const REPORT_OPEN_ID: &str = "BTN_REPORT_OPEN";
const REPORT_DOWN_ID: &str = "BTN_REPORT_DOWN";
const REPORT_BLOCKED_ID: &str = "BTN_REPORT_BLOCKED";

async fn handle_interaction(
    interaction: &serenity::MessageComponentInteraction,
    user_reports: mpsc::Sender<UserReport>,
    http: &serenity::Http,
    shard: &serenity::ShardMessenger,
) -> Result<(), Error> {
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
        .create_interaction_response(http, replace_builder_with(modal))
        .await?;

    let Some(response) = serenity::CollectModalInteraction::new(shard)
        .filter(move |modal_interaction| modal_interaction.data.custom_id == modal_id)
        .timeout(Duration::from_secs(60 * 60))
        .await
        else { return Ok(())};

    response
        .create_interaction_response(http, |res| {
            res.kind(serenity::InteractionResponseType::DeferredUpdateMessage)
        })
        .await?;

    let modal_information =
        ReportModal::parse(response.data.clone()).map_err(serenity::Error::Other)?;

    let escalators = match EscalatorInput::try_from(modal_information) {
        Ok(e) => e,
        Err(err) => {
            interaction
                .create_followup_message(http, |msg| msg.content(err.to_string()).ephemeral(true))
                .await?;

            return Ok(());
        }
    };

    let reporter = interaction.member.as_ref().map(|member| member.user.id);

    let report = UserReport {
        escalators,
        status,
        reporter,
    };
    user_reports.send(report).await?;

    let message = format!(
        "You've successfully reported {} as `{}`",
        escalators.message_noun(),
        format!("{status:?}").to_uppercase()
    );

    interaction
        .create_followup_message(http, |msg| msg.content(message).ephemeral(true))
        .await?;

    Ok(())
}

pub const REPORT_EMOJI: char = '📢';

/// Generate report button components with the proper IDs.
pub fn add_report_buttons(
    components: &mut serenity::CreateComponents,
) -> &mut serenity::CreateComponents {
    // TODO: DRY, this is unecessarily verbose
    components.create_action_row(|row| {
        row.create_button(|btn| {
            btn.label("OPEN")
                .emoji(REPORT_EMOJI)
                .style(serenity::ButtonStyle::Success)
                .custom_id(REPORT_OPEN_ID)
        })
        .create_button(|btn| {
            btn.label("DOWN")
                .emoji(REPORT_EMOJI)
                .style(serenity::ButtonStyle::Danger)
                .custom_id(REPORT_DOWN_ID)
        })
        .create_button(|btn| {
            btn.label("BLOCKED")
                .emoji(REPORT_EMOJI)
                .style(serenity::ButtonStyle::Secondary)
                .custom_id(REPORT_BLOCKED_ID)
        })
    })
}

impl Drop for MenuHandle {
    fn drop(&mut self) {
        self.interaction_task.abort();
    }
}
