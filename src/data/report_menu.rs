use crate::{interaction::add_report_buttons, prelude::*};

use super::Statuses;

use shuttle_persist::PersistInstance;

pub struct ReportMenu {
    message: Option<serenity::Message>,
    should_save: bool,
}

impl ReportMenu {
    pub fn is_initialized(&self) -> bool {
        self.message.is_some()
    }

    pub async fn update<F>(
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

    pub fn load_persist(persist: &PersistInstance) -> Self {
        let (message, should_save) = persist
            .load::<Option<serenity::Message>>("report_menu")
            .map(|msg| (msg, false))
            .unwrap_or_else(|_| (None, true));

        Self {
            message,
            should_save,
        }
    }

    pub fn save_persist(&mut self, persist: &PersistInstance) {
        if self.should_save {
            let _ = persist.save("report_menu", &self.message).ok();
            self.should_save = false;
        }
    }
}
