use crate::{interaction::add_report_buttons, prelude::*};

use super::Statuses;

use shuttle_persist::PersistInstance;

#[derive(Debug)]
pub struct ReportMenu {
    message: Option<serenity::Message>,
    should_save: bool,
}

impl ReportMenu {
    pub fn is_initialized(&self) -> bool {
        self.message.is_some()
    }

    /// Tries to update the report menu message.
    ///
    /// Only returns an error if there was an error trying to edit the message.
    pub async fn update(
        &mut self,
        cache_http: impl serenity::CacheHttp,
        statuses: &Statuses,
    ) -> Result<(), Error> {
        let Some(message) = &mut self.message else { return Ok(()) };

        message
            .edit(cache_http, |msg| msg.content(statuses.menu_message()))
            .await?;

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

        self.message = Some(handle.into_message().await?);
        self.should_save = true;

        Ok(())
    }

    pub async fn clear(&mut self, ctx: Context<'_>) -> Result<(), Error> {
        let Some(message) = self.message.take() else {
            ctx.say("Report menu is already cleared.").await?;
            return Ok(());
        };

        self.should_save = true;

        // ignore failure
        let _ = message.delete(ctx).await.ok();

        ctx.say("Report menu successfully cleared from memory.")
            .await?;

        Ok(())
    }

    pub async fn load_persist(
        http: impl AsRef<serenity::Http>,
        persist: &PersistInstance,
    ) -> Result<Self, Error> {
        let Some(ids) = persist.load::<Option<(u64, u64)>>("report_menu")? else {
            return Ok(Self::new(None, false));
        };

        let channel_id = serenity::ChannelId(ids.0);
        let message_id = serenity::MessageId(ids.1);
        let message = channel_id.message(http, message_id).await?;

        Ok(Self::new(Some(message), false))
    }

    pub fn save_persist(&mut self, persist: &PersistInstance) {
        if self.should_save {
            let ids = self
                .message
                .as_ref()
                .map(|msg| (msg.channel_id.0, msg.id.0));

            let _ = persist.save("report_menu", ids).ok();

            self.should_save = false;
        }
    }

    fn new(message: Option<serenity::Message>, should_save: bool) -> Self {
        Self {
            message,
            should_save,
        }
    }
}

impl Default for ReportMenu {
    fn default() -> Self {
        Self::new(None, true)
    }
}
