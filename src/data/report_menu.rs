use crate::prelude::*;

use super::Statuses;

use shuttle_persist::PersistInstance;

pub struct ReportMenu {
    message: Option<serenity::Message>,
    should_save: bool,
}

impl ReportMenu {
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
