use crate::{
    bot_tasks::BotTask,
    data::status::Status,
    generate::{INFO_BUTTON_ID, REPORT_EMOJI},
    prelude::*,
    ComponentMessage,
};

use indexmap::{indexmap, IndexMap};
use lazy_static::lazy_static;
use poise::serenity_prelude::{
    CacheHttp, CreateEmbed, CreateInteractionResponse, CreateInteractionResponseMessage,
};
use std::sync::Arc;
use tokio::sync::broadcast::{self, error::RecvError};

pub struct InfoTask;

pub struct TaskData<T> {
    interactions: broadcast::Receiver<Arc<ComponentMessage>>,
    cache_http: Arc<T>,
}

lazy_static! {
    static ref INFO_FIELDS: IndexMap<&'static str, String> = {
        indexmap! {
            "Escalators" => indoc::indoc! {"
                Every escalator can be identified by their starting and ending floors in the `#-#` format.
                For example, the escalator for going from the 4th floor to the 2nd floor has the label `4-2`.
            "}.to_owned(),
            "Statuses" => indoc::formatdoc! {"
                Next to each escalator is an emoji representing their current status.
                There are three different states it could be in:
                `{open} OPEN` - the escalator is in working condition.
                `{down} DOWN` - the escalator isn't moving, but can be walked on.
                `{blocked} BLOCKED` - the escalator is under maintenance and can't be walked on.
                ",
                open = Status::Open.emoji(),
                down = Status::Down.emoji(),
                blocked = Status::Blocked.emoji(),
            },
            "Reports" => indoc::formatdoc! {"
                You can report a status of an escalator by clicking the `{report} Report` button.
                Once in the report menu, you can specify the escalator by selecting \
                the starting and ending floors, and then you can select the status \
                and submit the report.
                You can also select `Pair` to report both the up and down escalators \
                or `All` if every escalator goes down due to an emergency (eg. a fire).
                ",
                report = REPORT_EMOJI,
            },
        }
    };
}

impl<T: CacheHttp + 'static> BotTask<T> for InfoTask {
    type Data = TaskData<T>;
    type Term = anyhow::Result<()>;

    async fn setup(&self, data: &Data, cache_http: Arc<T>) -> Option<Self::Data> {
        Some(TaskData {
            interactions: data.receiver(),
            cache_http,
        })
    }

    async fn run(self, mut data: Self::Data) -> Self::Term {
        loop {
            let event = match data.interactions.recv().await {
                Ok(event) if event.interaction.data.custom_id == INFO_BUTTON_ID => event,
                Ok(_) => continue,
                Err(RecvError::Closed) => return Ok(()),
                Err(RecvError::Lagged(n)) => {
                    log::warn!("Interaction receiver lagged behind by {n} values.");
                    continue;
                }
            };

            let embed = CreateEmbed::new()
                .title("What The Heck Does All Of This Mean?")
                .fields(
                    INFO_FIELDS
                        .iter()
                        .map(|(title, desc)| (*title, desc, false)),
                );

            let msg = CreateInteractionResponseMessage::new()
                .embed(embed)
                .ephemeral(true);

            let res = CreateInteractionResponse::Message(msg);

            event
                .interaction
                .create_response(&data.cache_http, res)
                .await?;
        }
    }
}
