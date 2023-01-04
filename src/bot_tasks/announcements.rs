use std::{collections::HashSet, sync::Arc, time::Duration};

use crate::{
    data::{Statuses, Update, UserReport, UNKNOWN_STATUS_EMOJI},
    prelude::*,
};

use itertools::{Either, Itertools};
use tokio::{sync::broadcast, task::JoinHandle, time::Instant};

use super::BotTask;

/// How much time to wait to send the history after receiving an update
/// if there hasn't been one in the past MAX_INTERVAL.
const MIN_INTERVAL: Duration = Duration::from_secs(60);

/// The most amount of time to wait to send the history after receiving an update.
const MAX_INTERVAL: Duration = Duration::from_secs(5 * 60);

pub struct AnnouncementTask(pub broadcast::Receiver<Update>);

impl BotTask for AnnouncementTask {
    fn begin(mut self, framework: Arc<poise::Framework<Data, Error>>) -> JoinHandle<()> {
        let cache_http = Arc::clone(&framework.client().cache_and_http);

        tokio::spawn(async move {
            let data = framework.user_data().await;
            let statuses = &data.statuses;
            let history_channel = &data.history_channel;

            let mut last_announcement = Instant::now();

            // it might be a good idea to add a check to see if there is a history
            // channel set, so it doesn't do all of this work if it doesn't need to.
            loop {
                // wait until an update is received
                let first_update = match self.0.recv().await {
                    Ok(update) => update,
                    // if the channel closed (for some reason) then stop the loop
                    Err(broadcast::error::RecvError::Closed) => break,
                    // if the receiver is lagging beind, restart the loop and try receiving again
                    // TODO: log channel lag
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                };

                let now = Instant::now();

                let time_since_last = now.duration_since(last_announcement);
                let delay = if time_since_last + MIN_INTERVAL < MAX_INTERVAL {
                    MAX_INTERVAL - time_since_last
                } else {
                    MIN_INTERVAL
                };

                let sleep = tokio::time::sleep(delay);
                tokio::pin!(sleep);

                // continue to save the updates until the interval is up
                let mut history = vec![first_update];
                loop {
                    tokio::select! {
                        Ok(next_update) = self.0.recv() => {
                            history.push(next_update);
                            continue;
                        }
                        () = &mut sleep => break,
                    }
                }

                // get summary of the current escalator statuses
                let mut embed = statuses.lock().await.gist();
                embed.timestamp(chrono::Utc::now());

                // separate the user reports from the status decays
                let (reports, outdated): (Vec<_>, Vec<_>) =
                    history.into_iter().partition_map(partition_update);

                // collect all reported escalators into a set
                let reported_escalators = reports
                    .iter()
                    .map(|report| report.escalators)
                    .flat_map(Vec::from)
                    .collect::<HashSet<_>>();

                // filter out outdated statuses that have recently been reported
                let outdated = outdated
                    .into_iter()
                    .filter(|escalator| !reported_escalators.contains(escalator))
                    .sorted()
                    .collect::<Vec<_>>();

                // add report history
                if !reports.is_empty() {
                    let message = format_reports(reports.into_iter().rev());
                    embed.field("Recent reports (newest first)", message, false);
                }

                // add outdated
                if !outdated.is_empty() {
                    fn make_ascii_titlecase(s: &mut str) {
                        if let Some(r) = s.get_mut(0..1) {
                            r.make_ascii_uppercase();
                        }
                    }

                    let mut escalators = Statuses::nounify_escalators(&outdated);
                    make_ascii_titlecase(&mut escalators);

                    let mut message = format!("`{}` {}", UNKNOWN_STATUS_EMOJI, escalators);

                    if outdated.len() == 1 {
                        message.push_str(" has ");
                    } else {
                        message.push_str(" have ");
                    }

                    message.push_str("been marked as `UNKNOWN` due to infrequent reports.");

                    embed.field("Outdated", message, false);
                }

                // send embed to history channel
                let res = history_channel
                    .read()
                    .await
                    .send(&cache_http.http, move |msg| {
                        msg.embed(replace_builder_with(embed))
                    })
                    .await;

                match res {
                    Ok(Some(msg)) => {
                        let _ = msg.crosspost(&cache_http).await.ok();
                    }
                    Ok(None) => (),
                    // TODO: handle error
                    Err(err) => println!("{err:?}"),
                }

                last_announcement = Instant::now();
            }
        })
    }
}

const MAX_REPORTS_DISPLAYED: usize = 14;

fn format_reports<I>(reports: I) -> String
where
    I: Iterator<Item = UserReport> + ExactSizeIterator,
{
    let mut reports = reports.map(|report| report.to_string());

    if reports.len() <= MAX_REPORTS_DISPLAYED {
        return reports.join("\n");
    }

    let mut message = String::new();
    for report in reports.by_ref().take(MAX_REPORTS_DISPLAYED - 1) {
        message.push_str(&report);
        message.push('\n');
    }

    message.push_str(&format!("\n*(...and {} more)*", reports.len()));

    message
}

fn partition_update(update: Update) -> Either<UserReport, (u8, u8)> {
    match update {
        Update::Report { report, .. } => Either::Left(report),
        Update::Outdated(escalator) => Either::Right(escalator),
    }
}
