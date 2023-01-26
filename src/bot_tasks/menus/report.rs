use crate::{prelude::*, bot_tasks::BotTask, data::{report::UserReport, escalator_input::EscalatorInput, status::Status}, generate::REPORT_BUTTON_ID, ComponentMessage};

use futures::{StreamExt, TryStreamExt};
use poise::async_trait;
use tokio::sync::broadcast::{self, error::RecvError};
use std::{sync::Arc, str::FromStr, time::Duration};

pub struct ReportTask;

pub struct TaskData {
    pool: sqlx::PgPool,
    interactions: broadcast::Receiver<Arc<ComponentMessage>>,
    reporter: broadcast::Sender<UserReport>,
    cache_http: Arc<serenity::CacheAndHttp>,
}

struct ReportComponent {

}

#[derive(Clone, Copy)]
struct Report {
    escalators: EscalatorInput,
    status: Status,
}

enum ComponentStatus<T> {
    Continue,
    Complete(T),
}

enum ComponentAction {
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
            pool: data.pool.clone(),
            interactions: data.receiver(),
            reporter: data.sender(),
            cache_http,
        })
    }

    async fn run(self, mut data: Self::Data) -> Self::Term {
        loop {
            let event = match data.interactions.recv().await {
                Ok(event) if event.interaction.data.custom_id == REPORT_BUTTON_ID => event,
                Ok(_) => continue,
                Err(RecvError::Closed) => return Ok(()),
                Err(RecvError::Lagged(n)) => {
                    log::warn!("Interaction receiver lagged behind by {n} values.");
                    continue;
                }
            };

            let pool = data.pool.clone();
            let http = Arc::clone(&data.cache_http.http);
            let reporter = data.reporter.clone();

            tokio::spawn(async move {
                if let Err(err) = handle_report(&pool, &http, &event, reporter).await {
                    log::warn!("An error ocurred while handling report: {err}");
                }
            });
        }
    }
}

async fn handle_report(
    pool: &sqlx::PgPool,
    http: &serenity::Http,
    event: &ComponentMessage,
    reporter: broadcast::Sender<UserReport>,
) -> Result<(), Error> {
    let mut report = ReportComponent::new();

    event.interaction
        .create_interaction_response(&http, |res| {
            res.interaction_response_data(|data| {
                data.set_components(report.render()).ephemeral(true)
            })
        })
        .await?;

    let mut actions = event.interaction
        .get_interaction_response(http)
        .await?
        .await_component_interactions(&event.shard)
        .build();

    let res = loop {
        let sleep = tokio::time::sleep(Duration::from_secs(2 * 60));
        tokio::pin!(sleep);

        let action = tokio::select! {
            Some(action) = actions.next() => action,
            _ = sleep => break None,
        };

        action.defer(http).await?;

        let command = match action.data.custom_id.parse::<ComponentAction>() {
            Ok(command) => command,
            Err(err) => {
                log::warn!("An error ocurred parsing a component command: {err}");
                continue;
            }
        };

        if let ComponentStatus::Complete(report) = report.execute(command) {
            break Some(report);
        }

        let replace_components = replace_builder_with(report.render());

        event.interaction
            .edit_original_interaction_response(http, |res| {
                res.components(replace_components)
            })
            .await?;
    };

    actions.stop();

    event.interaction.edit_original_interaction_response(http, |msg| {
        msg.components(|components| components.set_action_rows(vec![]))
    }).await?;

    let Some(report) = res else {
        log::debug!("Interaction timed out.");

        event.interaction.edit_original_interaction_response(http, |msg| {
            msg.content("Interaction timed out, try again...")
        }).await?;

        return Ok(());
    };

    let affected_escalators = match commit_report(pool, report).await {
        Ok(escalators) => escalators,
        Err(err) => {
            log::error!("An error ocurred trying to update statuses: {err}");

            event.interaction.edit_original_interaction_response(http, |msg| {
                msg.content("A database error ocurred.")
            }).await?;

            return Ok(());
        }
    };

    let message = format!(
        "{} Successfully reported {}.",
        report.status.emoji(),
        report.escalators.message_noun(),
    );

    event.interaction.edit_original_interaction_response(http, |msg| {
        msg.content(message)
    }).await?;

    let full_report = UserReport {
        reporter: Some(event.interaction.user.id),
        affected_escalators,
        escalators: report.escalators,
        new_status: report.status,
    };

    let _ = reporter.send(full_report).ok();

    Ok(())
}


async fn commit_report(
    pool: &sqlx::PgPool,
    report: Report,
) -> Result<smallvec::SmallVec<[EscalatorFloors; 2]>, sqlx::Error> {
    let status = report.status;

    match report.escalators {
        EscalatorInput::All => report_all(pool, status).await,
        EscalatorInput::Direct(start, end) => {
            let floors = EscalatorFloors::new(start, end);
            let escalator = Escalator { floors, status };

            Ok(if report_escalator(pool, escalator).await? {
                smallvec::smallvec![floors]
            } else {
                smallvec::smallvec![]
            })
        }
        EscalatorInput::Pair(start, end) => {
            let mut transaction = pool.begin().await?;

            let mut escalators = smallvec::smallvec![];
            for (start, end) in [(start, end), (end, start)] {
                let floors = EscalatorFloors::new(start, end);
                let escalator = Escalator { floors, status };

                if report_escalator(&mut transaction, escalator).await? {
                    escalators.push(floors);
                }
            }

            transaction.commit().await?;

            Ok(escalators)
        }
    }
}

/// Updates every escalator's status,
/// returning all affected escalators.
async fn report_all(
    pool: &sqlx::PgPool,
    status: Status,
) -> Result<smallvec::SmallVec<[EscalatorFloors; 2]>, sqlx::Error> {
    sqlx::query_as::<_, EscalatorFloors>(
        "
        UPDATE escalators
        SET current_status = $1
        WHERE current_status <> $1
        RETURNING floor_start, floor_end
        "
    )
    .bind(status)
    .fetch(pool)
    .try_collect()
    .await
}

/// Attempts to update a specific escalator's status,
/// returning whether or not the escalator exists and if it changed the status.
async fn report_escalator(
    pool: impl sqlx::Executor<'_, Database = sqlx::Postgres>,
    escalator: Escalator,
) -> Result<bool, sqlx::Error> {
    todo!()
}

impl ReportComponent {
    fn new() -> Self {
        Self {

        }
    }

    fn render(&self) -> serenity::CreateComponents {
        todo!()
    }

    fn execute(&mut self, command: ComponentAction) -> ComponentStatus<Report> {
        todo!()
    }
}

impl FromStr for ComponentAction {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        todo!()
    }
}
