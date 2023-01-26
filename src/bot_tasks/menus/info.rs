use crate::{prelude::*, bot_tasks::BotTask, generate::{INFO_BUTTON_ID, REPORT_EMOJI}, data::status::Status};

use indexmap::{IndexMap, indexmap};
use lazy_static::lazy_static;
use poise::async_trait;
use tokio::sync::broadcast::{self, error::RecvError};
use std::sync::Arc;

pub struct ReportTask;

type ComponentMessage = Arc<serenity::MessageComponentInteraction>;

pub struct TaskData {
    interactions: broadcast::Receiver<ComponentMessage>,
    cache_http: Arc<serenity::CacheAndHttp>,
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

#[async_trait]
impl BotTask for ReportTask {
    type Data = TaskData;
    type Term = anyhow::Result<()>;

    async fn setup(
        &self,
        framework: std::sync::Weak<poise::Framework<Data, Error>>,
    ) -> Option<Self::Data> {
        let framework = framework.upgrade()?;

        let cache_http = Arc::clone(&framework.client().cache_and_http);

        let data = framework.user_data().await;

        Some(TaskData {
            interactions: data.receiver(),
            cache_http,
        })
    }

    async fn run(self, mut data: Self::Data) -> Self::Term {
        loop {
            let interaction = match data.interactions.recv().await {
                Ok(interaction) if interaction.data.custom_id == INFO_BUTTON_ID => interaction,
                Ok(_) => continue,
                Err(RecvError::Closed) => return Ok(()),
                Err(RecvError::Lagged(n)) => {
                    log::warn!("Interaction receiver lagged behind by {n} values.");
                    continue;
                }
            };

            interaction
                .create_interaction_response(&data.cache_http.http, |res| {
                    res.interaction_response_data(|data| {
                        data.embed(|embed| {
                            embed
                                .title("What The Heck Does All Of This Mean?")
                                .fields(INFO_FIELDS.iter().map(|(title, desc)| (title, desc, false)))
                        })
                        .ephemeral(true)
                    })
                })
                .await?;
        }
    }
}
