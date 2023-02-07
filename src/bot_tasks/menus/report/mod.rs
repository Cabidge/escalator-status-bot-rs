mod component;

use crate::{
    bot_tasks::BotTask,
    data::{escalator_input::EscalatorInput, report::UserReport, status::Status},
    generate::REPORT_BUTTON_ID,
    prelude::*,
    ui::{
        MessageHandle, Timeout, TimeoutKind, UiConfig, UserInterface, message::{message_interaction::ToInteractionHandle}, View,
    },
    ComponentMessage,
};

use chrono::prelude::*;
use chrono_tz::America::New_York as NYCTimeZone;
use futures::TryStreamExt;
use poise::async_trait;
use std::{sync::Arc, time::Duration};
use tokio::sync::broadcast::{self, error::RecvError};

pub struct ReportTask;

pub struct TaskData {
    pool: sqlx::PgPool,
    interactions: broadcast::Receiver<Arc<ComponentMessage>>,
    reporter: broadcast::Sender<UserReport>,
    cache_http: Arc<serenity::CacheAndHttp>,
}

#[derive(Clone, Copy)]
pub struct Report {
    escalators: EscalatorInput,
    status: Status,
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

            let nyc_now = Utc::now().with_timezone(&NYCTimeZone);

            let is_weekday = nyc_now.weekday().num_days_from_monday() < 5;
            let is_active_time = (7..=18).contains(&nyc_now.hour());

            if !(is_weekday && is_active_time) {
                let _ = event.interaction.create_interaction_response(&data.cache_http.http, |res| {
                    res.interaction_response_data(|data| {
                        data.content("Reports are locked any time before 6 am, after 7 pm, and during weekends.")
                            .ephemeral(true)
                    })
                }).await.ok();

                continue;
            }

            let pool = data.pool.clone();
            let http = Arc::clone(&data.cache_http.http);
            let reporter = data.reporter.clone();

            tokio::spawn(async move {
                if let Err(err) = handle_report(&pool, http, &event, reporter).await {
                    log::warn!("An error ocurred while handling report: {err}");
                }
            });
        }
    }
}

async fn handle_report(
    pool: &sqlx::PgPool,
    http: Arc<serenity::Http>,
    event: &ComponentMessage,
    reporter: broadcast::Sender<UserReport>,
) -> Result<(), Error> {
    let report = component::ReportComponent::new();

    let timeout = Timeout {
        duration: Duration::from_secs(2 * 60),
        kind: TimeoutKind::Refresh,
    };

    let config = UiConfig {
        timeout: Some(timeout),
    };

    let mut ui = event
        .interaction
        .to_handle::<true>(&http)
        .into_ui(&http, &event.shard);

    let report = ui.mount(report, config).await?;

    let affected_escalators = match commit_report(pool, report).await {
        Ok(e) => e,
        Err(err) => {
            let response = View::with_content("A database error ocurred...");
            ui.handle.show(response).await?;

            return Err(err.into());
        }
    };

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
