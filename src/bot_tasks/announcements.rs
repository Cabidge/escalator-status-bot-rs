use std::{sync::Arc, time::Duration};

use crate::{
    data::{Statuses, Update, UserReport, UNKNOWN_STATUS_EMOJI, ReportKind},
    prelude::*,
};

use indexmap::IndexSet;
use itertools::Itertools;
use tokio::{sync::broadcast, task::JoinHandle, time::Instant};

use super::BotTask;

/// How much time to wait to send the history after receiving an update
/// if there hasn't been one in the past MAX_INTERVAL.
const MIN_INTERVAL: Duration = Duration::from_secs(60);

/// The most amount of time to wait to send the history after receiving an update.
const MAX_INTERVAL: Duration = Duration::from_secs(5 * 60);

pub struct AnnouncementTask(pub broadcast::Receiver<Update>);

#[derive(Default)]
struct AnnouncementBuilder {
    reports: Vec<UserReport>,
    outdated: IndexSet<Escalator>,
}

impl AnnouncementBuilder {
    const MAX_REPORTS_DISPLAYED: usize = 14;

    fn add_update(&mut self, update: Update) {
        match update {
            Update::Outdated(escalator) => {
                self.outdated.insert(escalator);
            }
            Update::Report { report, .. } => self.add_report(report),
        }
    }

    fn add_report(&mut self, report: UserReport) {
        self.reports.push(report);

        for escalator in report.escalators {
            self.outdated.shift_remove(&escalator);
        }
    }

    fn attach(self, embed: &mut serenity::CreateEmbed) {
        // add report history
        if !self.reports.is_empty() {
            let message = Self::format_reports(self.reports.into_iter().rev());
            embed.field("Recent reports (newest first)", message, false);
        }

        // add outdated
        if !self.outdated.is_empty() {
            fn make_ascii_titlecase(s: &mut str) {
                if let Some(r) = s.get_mut(0..1) {
                    r.make_ascii_uppercase();
                }
            }

            let outdated = self.outdated.into_iter().collect_vec();
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
    }

    fn format_reports<I>(reports: I) -> String
    where
        I: Iterator<Item = UserReport> + ExactSizeIterator,
    {
        let mut reports = reports.map(|report| report.to_string());

        if reports.len() <= Self::MAX_REPORTS_DISPLAYED {
            return reports.join("\n");
        }

        let mut message = String::new();
        for report in reports.by_ref().take(Self::MAX_REPORTS_DISPLAYED - 1) {
            message.push_str(&report);
            message.push('\n');
        }

        message.push_str(&format!("\n*(...and {} more)*", reports.len()));

        message
    }
}

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
            'announcement: loop {
                let mut announcement = AnnouncementBuilder::default();

                // wait until a significant update is received
                loop {
                    match self.0.recv().await {
                        Ok(Update::Report { report, kind: ReportKind::Redundant }) if announcement.reports.len() < 15 => {
                            log::debug!("Received redundant report, continuing...");
                            announcement.add_report(report);
                        },
                        Ok(update) => {
                            announcement.add_update(update);
                            break;
                        }
                        // if the channel closed (for some reason) then stop the loop
                        Err(broadcast::error::RecvError::Closed) => {
                            log::debug!("Update receiver has closed.");
                            break 'announcement;
                        }
                        // if the receiver is lagging beind, restart the loop and try receiving again
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            log::warn!("Update receiver has lagged by {n} updates.");
                        }
                    };
                }

                let now = Instant::now();

                let time_since_last = now.duration_since(last_announcement);
                let delay = if time_since_last + MIN_INTERVAL < MAX_INTERVAL {
                    MAX_INTERVAL - time_since_last
                } else {
                    MIN_INTERVAL
                };

                log::info!(
                    "Received update, pooling for {} seconds before announcing.",
                    delay.as_secs()
                );

                let sleep = tokio::time::sleep(delay);
                tokio::pin!(sleep);

                // continue to save the updates until the interval is up
                loop {
                    tokio::select! {
                        Ok(update) = self.0.recv() => announcement.add_update(update),
                        () = &mut sleep => break,
                    }
                }

                log::info!("Generating announcement...");

                // get summary of the current escalator statuses
                let mut embed = statuses.lock().await.gist();
                embed.timestamp(chrono::Utc::now());

                announcement.attach(&mut embed);

                // send embed to history channel
                log::info!("Sending announcement...");
                let res = history_channel
                    .read()
                    .await
                    .send(&cache_http.http, move |msg| {
                        msg.embed(replace_builder_with(embed))
                    })
                    .await;

                match res {
                    Ok(Some(msg)) => {
                        log::info!("Publishing announcement.");

                        if let Err(err) = msg.crosspost(&cache_http).await {
                            log::warn!("Publish error: {err:?}");
                        }
                    }
                    Ok(None) => log::info!("No history channel found."),
                    Err(err) => log::error!("Announcement error: {err:?}"),
                }

                last_announcement = Instant::now();
            }
        })
    }
}
