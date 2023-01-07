mod info;

pub use self::info::{ReportKind, UNKNOWN_STATUS_EMOJI};

use crate::prelude::*;

use indexmap::IndexMap;
use itertools::Itertools;
use shuttle_persist::PersistInstance;
use tokio::sync::broadcast;

use super::{status::Status, UserReport};
use info::Info;

pub const ESCALATOR_COUNT: usize = 14;

/// An array of all valid escalators.
pub const ESCALATORS: [Escalator; ESCALATOR_COUNT] = [
    (2, 3), // 0
    (2, 4), // 1
    (3, 2), // 2
    (3, 5), // 3
    (4, 2), // 4
    (4, 6), // 5
    (5, 3), // 6
    (5, 7), // 7
    (6, 4), // 8
    (6, 8), // 9
    (7, 5), // 10
    (7, 9), // 11
    (8, 6), // 12
    (9, 7), // 13
];

/// An array of all escalator indicies in "pair order,"
/// (ie. escalator pairs come one after another)
pub const PAIR_ORDER: [usize; ESCALATOR_COUNT] = [
    0, 2, // 2/3
    1, 4, // 2/4
    3, 6, // 3/5
    5, 8, // 4/6
    7, 10, // 5/7
    9, 12, // 6/8
    11, 13, // 7/9
];

#[derive(Debug, Clone)]
pub struct Statuses {
    escalators: IndexMap<Escalator, Info>,
    updates: broadcast::Sender<Update>,
    should_save: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum Update {
    Report {
        report: UserReport,
        kind: ReportKind,
    },
    Outdated(Escalator),
}

type Escalators = IndexMap<Escalator, Info>;

impl Statuses {
    fn new(
        escalators: Escalators,
        updates_tx: broadcast::Sender<Update>,
        should_save: bool,
    ) -> Self {
        Self {
            escalators,
            updates: updates_tx,
            should_save,
        }
    }

    pub fn load_persist(persist: &PersistInstance, update_tx: broadcast::Sender<Update>) -> Self {
        log::info!("Loading escalator statuses...");

        let (escalators, should_save) = persist
            .load::<Escalators>("escalators")
            .map(|escalators| (escalators, false)) // if load success, no need to save
            .unwrap_or_else(|err| {
                // if load failed, create default and save
                log::error!("Load error: {err:?}");
                (Self::default_escalators(), true)
            });

        Self::new(escalators, update_tx, should_save)
    }

    pub fn save_persist(&mut self, persist: &PersistInstance) {
        if !self.should_save {
            return;
        }

        log::info!("Saving escalator statuses...");

        if let Err(err) = persist.save("escalators", &self.escalators) {
            log::error!("Save error: {err:?}");
        }

        self.should_save = false;
    }

    pub fn menu_message(&self) -> String {
        String::from("**Escalator Statuses:**```\n")
            + &PAIR_ORDER
                .into_iter()
                .filter_map(|index| self.escalators.get_index(index))
                .map(|((start, end), info)| format!("{} {}-{}", info.status_emoji(), start, end))
                .chunks(2)
                .into_iter()
                .map(|mut pair| pair.join(" "))
                .join("\n")
            + "```"
    }

    /// Generates a summary for a specific status.
    fn summarize_status(status: Option<Status>, escalators: &[Escalator]) -> String {
        let emoji = match status {
            Some(status) => status.emoji(),
            None => UNKNOWN_STATUS_EMOJI,
        };

        let mut message = format!("`{}` ", emoji);

        if escalators.len() == ESCALATORS.len() {
            // all
            message.push_str("`ALL` escalators");
        } else if escalators.len() >= ESCALATORS.len() / 2 {
            // more than half
            message.push_str("`MANY` escalators");
        } else {
            // less than half
            message.push_str(&Self::nounify_escalators(escalators));
        }

        if escalators.len() == 1 {
            message.push_str(" is ");
        } else {
            message.push_str(" are ");
        }

        // TODO: make this less verbose
        let status = match status {
            Some(Status::Open) => "`OPEN`",
            Some(Status::Down) => "`DOWN`",
            Some(Status::Blocked) => "`BLOCKED`",
            None => "`UNKNOWN`",
        };

        message.push_str(status);
        message.push('.');

        message
    }

    /// Turn a collection of Escalators into a format that could be put into a message.
    pub fn nounify_escalators(escalators: &[Escalator]) -> String {
        if escalators.is_empty() {
            return String::from("`NO` escalators");
        }

        if escalators.len() == 1 {
            return format!("The {} escalator", Self::nounify_escalator(escalators[0]));
        }

        // how many escalators there are not including the first and last
        let mid_count = escalators.len() - 2;

        let mut escalators = escalators.iter().copied().map(Self::nounify_escalator);
        let mut noun = format!("The {}", escalators.next().unwrap());

        for escalator in escalators.by_ref().take(mid_count) {
            noun.push_str(&format!(", {escalator}"));
        }

        noun.push_str(&format!(", and {} escalators", escalators.next().unwrap()));

        noun
    }

    pub fn get_info(&self, escalator: Escalator) -> Option<&Info> {
        self.escalators.get(&escalator)
    }

    /// Turn an escalator into a format that could be put into a message.
    fn nounify_escalator((start, end): Escalator) -> String {
        format!("`{}-{}`", start, end)
    }

    fn escalators_with_status(&self, status: Option<Status>) -> Vec<Escalator> {
        self.escalators
            .iter()
            .filter_map(|(&escalator, info)| (info.status() == status).then_some(escalator))
            .collect()
    }

    /// Generate a summary of the statuses as an embed.
    pub fn gist(&self) -> serenity::CreateEmbed {
        // -- Setup

        let mut embed = serenity::CreateEmbed::default();

        embed.title("Here's the gist...");

        // -- Handle Variations

        if self.escalators.values().any(Info::is_out_of_order) {
            // -- Some are Down/Blocked

            embed.color((240, 60, 60));

            // add summaries for down and blocked status escalators (only if there are any of either)
            let description = [Status::Down, Status::Blocked]
                .into_iter()
                .filter_map(|status| {
                    let escalators = self.escalators_with_status(Some(status));
                    (!escalators.is_empty())
                        .then(|| Self::summarize_status(Some(status), &escalators))
                })
                .join("\n");

            embed.description(&description);

            return embed;
        }

        // -- All are Open/Unknown

        let unknowns = self.escalators_with_status(None);

        if unknowns.is_empty() {
            // -- All are Open
            let emoji = Status::Open.emoji();
            embed.description(format!("`{emoji}` `ALL` escalators are `OPEN`! 🥳 🎉"));
            embed.color((55, 220, 70));
            return embed;
        }

        // -- Some are Unknown

        embed.color((250, 190, 25));
        embed.description(Self::summarize_status(None, &unknowns));

        embed
    }

    /// Update a given escalator's status.
    pub fn report(&mut self, report: UserReport) {
        log::info!("Updating statuses based on report: {report:?}");

        // report each escalator and get the "most significant" report kind
        let report_kind = report
            .escalators
            .into_iter()
            .filter_map(|escalator| {
                self.escalators
                    .get_mut(&escalator)
                    .map(|info| info.update_status(report.status))
            })
            .max()
            .unwrap_or(ReportKind::Redundant);

        self.send_update(Update::Report {
            report,
            kind: report_kind,
        });

        if report_kind != ReportKind::Redundant {
            self.should_save = true;
        }
    }

    /// Checks if the last time each escalator was updated is beyond a given threshold,
    /// setting the status to None if it is.
    pub fn handle_outdated(&mut self) {
        log::info!("Updating outdated statuses...");

        let updates: Vec<_> = self
            .escalators
            .iter_mut()
            .filter_map(|(escalator, info)| {
                info.handle_outdated()
                    .then_some(Update::Outdated(*escalator))
            })
            .collect();

        self.should_save |= !updates.is_empty();

        for update in updates {
            self.send_update(update);
        }
    }

    fn send_update(&self, update: Update) {
        log::info!("Sending update...");

        if let Err(err) = self.updates.send(update) {
            log::error!("Update sending error: {err:?}");
        }
    }

    fn default_escalators() -> Escalators {
        let mut escalators = IndexMap::with_capacity(ESCALATORS.len());

        for escalator in ESCALATORS {
            escalators.insert(escalator, Info::default());
        }

        escalators
    }
}
