use crate::prelude::*;

use shuttle_persist::PersistInstance;

#[derive(Debug)]
pub struct HistoryChannel {
    channel: Option<serenity::GuildChannel>,
    should_save: bool,
}

impl HistoryChannel {
    pub async fn send<F>(&self, cache_http: impl serenity::CacheHttp, f: F) -> Result<(), Error>
    where
        for<'a, 'b> F:
            FnOnce(&'b mut serenity::CreateMessage<'a>) -> &'b mut serenity::CreateMessage<'a>,
    {
        let Some(channel) = &self.channel else { return Ok(()) };
        channel.send_message(cache_http, f).await?;

        Ok(())
    }

    pub fn load_persist(persist: &PersistInstance) -> Self {
        let (channel, should_save) = persist
            .load::<Option<serenity::GuildChannel>>("history_channel")
            .map(|msg| (msg, false))
            .unwrap_or_else(|_| (None, true));

        Self {
            channel,
            should_save,
        }
    }

    pub fn save_persist(&mut self, persist: &PersistInstance) {
        if self.should_save {
            let _ = persist.save("history_channel", &self.channel).ok();
            self.should_save = false;
        }
    }
}
