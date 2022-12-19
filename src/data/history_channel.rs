use crate::prelude::*;

use shuttle_persist::PersistInstance;

#[derive(Debug)]
pub struct HistoryChannel {
    channel: Option<serenity::ChannelId>,
    should_save: bool,
}

impl HistoryChannel {
    pub async fn send<F>(&self, http: impl AsRef<serenity::Http>, f: F) -> Result<(), Error>
    where
        for<'a, 'b> F:
            FnOnce(&'b mut serenity::CreateMessage<'a>) -> &'b mut serenity::CreateMessage<'a>,
    {
        let Some(channel) = &self.channel else { return Ok(()) };
        channel.send_message(http, f).await?;

        Ok(())
    }

    pub fn load_persist(persist: &PersistInstance) -> Result<Self, Error> {
        let id = persist.load::<Option<u64>>("history_channel")?;
        let channel = id.map(serenity::ChannelId);
        Ok(Self::new(channel, false))
    }

    pub fn save_persist(&mut self, persist: &PersistInstance) {
        if self.should_save {
            let raw_id = self.channel.map(|channel| channel.0);
            let _ = persist.save("history_channel", raw_id).ok();
            self.should_save = false;
        }
    }

    fn new(channel: Option<serenity::ChannelId>, should_save: bool) -> Self {
        Self {
            channel,
            should_save,
        }
    }
}

impl Default for HistoryChannel {
    fn default() -> Self {
        Self::new(None, true)
    }
}
