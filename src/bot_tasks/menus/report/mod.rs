mod component;

use crate::{
    bot_tasks::BotTask,
    data::{escalator_input::EscalatorInput, report::UserReport, status::Status},
    generate::{self, REPORT_BUTTON_ID},
    prelude::*,
    ComponentMessage,
};

use chrono::prelude::*;
use chrono_tz::America::New_York as NYCTimeZone;
use futures::{StreamExt, TryStreamExt};
use poise::serenity_prelude::{
    CacheHttp, CreateInteractionResponse, CreateInteractionResponseMessage, EditInteractionResponse,
};
use std::{sync::Arc, time::Duration};
use tokio::sync::broadcast::{self, error::RecvError};

pub struct ReportTask;

pub struct TaskData<T> {
    pool: sqlx::PgPool,
    interactions: broadcast::Receiver<Arc<ComponentMessage>>,
    reporter: broadcast::Sender<UserReport>,
    cache_http: Arc<T>,
}

#[derive(Clone, Copy)]
pub struct Report {
    escalators: EscalatorInput,
    status: Status,
}

impl<T: CacheHttp + 'static> BotTask<T> for ReportTask {
    type Data = TaskData<T>;
    type Term = anyhow::Result<()>;

    async fn setup(&self, data: &Data, cache_http: Arc<T>) -> Option<Self::Data> {
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

            log::info!("Received REPORT interaction");

            let nyc_now = Utc::now().with_timezone(&NYCTimeZone);

            let is_weekday = nyc_now.weekday().num_days_from_monday() < 5;
            let is_active_time = (7..=18).contains(&nyc_now.hour());

            if !(is_weekday && is_active_time) {
                let msg = CreateInteractionResponseMessage::new()
                    .content(
                        "Reports are locked any time before 6 am, after 7 pm, and during weekends.",
                    )
                    .ephemeral(true);

                let res = CreateInteractionResponse::Message(msg);
                let _ = event
                    .interaction
                    .create_response(&data.cache_http, res)
                    .await
                    .ok();

                continue;
            }

            let pool = data.pool.clone();
            let http = Arc::clone(&data.cache_http);
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
    http: &impl CacheHttp,
    event: &ComponentMessage,
    reporter: broadcast::Sender<UserReport>,
) -> Result<(), Error> {
    const TIMEOUT: Duration = Duration::from_secs(2 * 60);

    let mut report = component::ReportComponent::new();

    let msg = CreateInteractionResponseMessage::new()
        .content(generate::timeout_message(TIMEOUT))
        .components(report.render())
        .ephemeral(true);
    let res = CreateInteractionResponse::Message(msg);
    event.interaction.create_response(&http, res).await?;

    let mut actions = event
        .interaction
        .get_response(http.http())
        .await?
        .await_component_interactions(&event.shard)
        .stream();

    let res = loop {
        let sleep = tokio::time::sleep(TIMEOUT);
        tokio::pin!(sleep);

        let action = tokio::select! {
            Some(action) = actions.next() => action,
            _ = sleep => break None,
        };

        action.defer(http).await?;

        let command = match action.data.custom_id.parse::<component::ComponentAction>() {
            Ok(command) => command,
            Err(err) => {
                log::warn!("An error ocurred parsing a component command: {err}");
                continue;
            }
        };

        if let component::ComponentStatus::Complete(report) = report.execute(command) {
            break Some(report);
        }

        let edit = EditInteractionResponse::new()
            .content(generate::timeout_message(TIMEOUT))
            .components(report.render());

        event.interaction.edit_response(http, edit).await?;
    };

    drop(actions);

    let edit = EditInteractionResponse::new()
        .content("Processing...")
        .components(vec![]);
    event.interaction.edit_response(http, edit).await?;

    let Some(report) = res else {
        log::debug!("Interaction timed out.");

        let edit = EditInteractionResponse::new().content("Interaction timed out, try again...");
        event.interaction.edit_response(http, edit).await?;

        return Ok(());
    };

    let affected_escalators = match commit_report(pool, report).await {
        Ok(escalators) => escalators,
        Err(err) => {
            log::error!("An error ocurred trying to update statuses: {err}");

            let edit = EditInteractionResponse::new().content("A database error ocurred.");
            event.interaction.edit_response(http, edit).await?;

            return Ok(());
        }
    };

    let message = format!(
        "`{}` Successfully reported {}.",
        report.status.emoji(),
        report.escalators.message_noun(),
    );

    let edit = EditInteractionResponse::new().content(message);
    event
        .interaction
        .edit_response(http, edit)
        .await?;

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

                if report_escalator(&mut *transaction, escalator).await? {
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
        ",
    )
    .bind(status)
    .fetch(pool)
    .try_collect()
    .await
}

/// Attempts to update a specific escalator's status,
/// returning whether or not the escalator exists and if it changed the status.
async fn report_escalator(
    executor: impl sqlx::Executor<'_, Database = sqlx::Postgres>,
    escalator: Escalator,
) -> Result<bool, sqlx::Error> {
    sqlx::query(
        "
        UPDATE escalators
        SET current_status = $1
        WHERE current_status <> $1
        AND floor_start = $2
        AND floor_end = $3
        RETURNING 1
        ",
    )
    .bind(escalator.status)
    .bind(escalator.floors.start as i16)
    .bind(escalator.floors.end as i16)
    .fetch_optional(executor)
    .await
    .map(|opt| opt.is_some())
}
