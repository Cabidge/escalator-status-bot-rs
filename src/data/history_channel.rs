use crate::prelude::*;

use shuttle_persist::PersistInstance;

#[derive(Debug)]
pub struct HistoryChannel {
    channel: Option<serenity::ChannelId>,
    should_save: bool,
}

pub struct InvalidChannelError;

impl HistoryChannel {
    pub async fn send<F>(
        &self,
        http: impl AsRef<serenity::Http>,
        f: F,
    ) -> Result<Option<serenity::Message>, Error>
    where
        for<'a, 'b> F:
            FnOnce(&'b mut serenity::CreateMessage<'a>) -> &'b mut serenity::CreateMessage<'a>,
    {
        let Some(channel) = &self.channel else { return Ok(None) };
        Ok(Some(channel.send_message(http, f).await?))
    }

    pub fn set(
        &mut self,
        channel: serenity::Channel,
    ) -> Result<serenity::ChannelId, InvalidChannelError> {
        let Some(channel) = channel.guild().filter(serenity::GuildChannel::is_text_based) else {
            return Err(InvalidChannelError);
        };

        if self.channel != Some(channel.id) {
            self.channel = Some(channel.id);
            self.should_save = true;
        }

        Ok(channel.id)
    }

    pub fn unset(&mut self) {
        if self.channel.take().is_some() {
            self.should_save = true;
        }
    }

    pub fn load_persist(persist: &PersistInstance) -> Self {
        log::info!("Loading history channel...");

        let id = match persist.load::<Option<u64>>("history_channel") {
            Ok(id) => id,
            Err(err) => {
                log::error!("Load error: {err:?}");
                return Self::new(None, true);
            }
        };

        let channel = id.map(serenity::ChannelId);
        Self::new(channel, false)
    }

    pub fn save_persist(&mut self, persist: &PersistInstance) {
        if !self.should_save {
            return;
        }

        log::info!("Saving history channel...");

        let raw_id = self.channel.map(|channel| channel.0);
        if let Err(err) = persist.save("history_channel", raw_id) {
            log::error!("Save error: {err:?}");
        }

        self.should_save = false;
    }

    fn new(channel: Option<serenity::ChannelId>, should_save: bool) -> Self {
        Self {
            channel,
            should_save,
        }
    }
}
