mod info;

use crate::prelude::*;

use indexmap::IndexMap;
use itertools::Itertools;
use shuttle_persist::PersistInstance;
use std::{fmt::Display, str::FromStr};
use tokio::sync::broadcast;

use self::info::UNKNOWN_STATUS_EMOJI;

use super::status::Status;
use info::Info;

pub const ESCALATORS: [Escalator; 14] = [
    (2, 3),
    (2, 4),
    (3, 2),
    (3, 5),
    (4, 2),
    (4, 6),
    (5, 3),
    (5, 7),
    (6, 4),
    (6, 8),
    (7, 5),
    (7, 9),
    (8, 6),
    (9, 7),
];

type Escalator = (u8, u8);

#[derive(Debug, Clone)]
pub struct Statuses {
    escalators: IndexMap<Escalator, Info>,
    updates: broadcast::Sender<Update>,
    should_save: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum Update {
    Report {
        escalators: EscalatorInput,
        status: Status,
        reporter: Option<serenity::UserId>,
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

    pub fn load_persist(update_tx: broadcast::Sender<Update>, persist: &PersistInstance) -> Self {
        let (escalators, should_save) = persist
            .load::<Escalators>("escalators")
            .map(|escalators| (escalators, false)) // if load success, no need to save
            .unwrap_or_else(|_| (Self::default_escalators(), true)); // if load failed, create default and save

        Self::new(escalators, update_tx, should_save)
    }

    pub fn save_persist(&mut self, persist: &PersistInstance) {
        // TODO: log error
        if self.should_save {
            let _ = persist.save("escalators", &self.escalators).ok();
            self.should_save = false;
        }
    }

    pub fn menu_message(&self) -> String {
        let mut msg = String::from("**Escalator Statuses:**```py\n");

        for (escalator, info) in self.escalators.iter() {
            // display a different emoji depending on the status
            msg.push(info.status_emoji());
            msg.push(' ');

            // if the status is open, surround escalator with quotes,
            // otherwise use hashtags
            let delimiter = if info.status() == Some(Status::Open) {
                '"'
            } else {
                '#'
            };

            // escalator label
            msg.push(delimiter);
            msg.push((escalator.0 + b'0') as char);
            msg.push('-');
            msg.push((escalator.1 + b'0') as char);
            msg.push(delimiter);

            msg.push('\n');
        }

        // close the code block
        msg.push_str("```");

        msg
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
    fn nounify_escalators(escalators: &[Escalator]) -> String {
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
            let summaries = [Status::Down, Status::Blocked]
                .into_iter()
                .filter_map(|status| {
                    let escalators = self.escalators_with_status(Some(status));
                    (!escalators.is_empty())
                        .then(|| Self::summarize_status(Some(status), &escalators))
                });

            // annoying to have to do this to avoid possible name collisions...
            // I just hope iter::intersperse get's stabilized soon so I can change this
            //
            // adds new lines between each summary for readability
            let description: String =
                Itertools::intersperse(summaries, String::from("\n")).collect();

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
    pub fn report(
        &mut self,
        escalators: EscalatorInput,
        status: Status,
        reporter: Option<serenity::UserId>,
    ) {
        let mut any_updated = false;
        // for each reported escalator, check if any of them successfully updated
        for escalator in Vec::<_>::from(escalators) {
            if let Some(info) = self.escalators.get_mut(&escalator) {
                any_updated |= info.update_status(status);
            }
        }

        if any_updated {
            // TODO: log error
            let update = Update::Report {
                escalators,
                status,
                reporter,
            };
            let _ = self.updates.send(update).ok();
            self.should_save = true;
        }
    }

    /// Checks if the last time each escalator was updated is beyond a given threshold,
    /// setting the status to None if it is.
    pub fn handle_outdated(&mut self) {
        for (escalator, info) in self.escalators.iter_mut() {
            if info.handle_outdated() {
                // TODO: log error
                let _ = self.updates.send(Update::Outdated(*escalator)).ok();
                self.should_save = true;
            }
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

#[derive(Debug, Clone, Copy)]
pub enum EscalatorInput {
    All,            // "all"
    Pair(u8, u8),   // "#/#"
    Direct(u8, u8), // "#-#"
}

#[derive(Debug, Clone, Copy)]
pub enum InputError {
    UnknownFormat,
    InvalidFloor(char),
    InvalidEscalator(u8, u8),
}

fn parse_floor(ch: char) -> Result<u8, InputError> {
    match ch {
        '1'..='9' => Ok(ch as u8 - b'0'),
        floor => Err(InputError::InvalidFloor(floor)),
    }
}

impl EscalatorInput {
    pub fn message_noun(&self) -> String {
        match self {
            Self::All => String::from("`ALL` escalators"),
            Self::Pair(a, b) => {
                let lower = a.min(b);
                let upper = a.max(b);
                format!("the `{lower}-{upper}` and `{upper}-{lower}` escalators")
            }
            Self::Direct(lower, upper) => format!("the `{lower}-{upper}` escalator"),
        }
    }
}

impl From<EscalatorInput> for Vec<Escalator> {
    fn from(value: EscalatorInput) -> Self {
        match value {
            EscalatorInput::All => ESCALATORS.into_iter().collect(),
            EscalatorInput::Pair(a, b) => vec![(a, b), (b, a)],
            EscalatorInput::Direct(a, b) => vec![(a, b)],
        }
    }
}

impl FromStr for EscalatorInput {
    type Err = InputError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 3 {
            return Err(InputError::UnknownFormat);
        }

        if s.to_lowercase() == "all" {
            return Ok(EscalatorInput::All);
        }

        let mut chars = s.chars();
        let left = chars.next().unwrap();
        let sep = chars.next().unwrap();
        let right = chars.next().unwrap();

        if sep != '-' && sep != '/' {
            return Err(InputError::UnknownFormat);
        }

        let left = parse_floor(left)?;
        let right = parse_floor(right)?;

        if !ESCALATORS.contains(&(left, right)) {
            return Err(InputError::InvalidEscalator(left, right));
        }

        match sep {
            '/' => Ok(EscalatorInput::Pair(left, right)),
            '-' => Ok(EscalatorInput::Direct(left, right)),
            _ => unreachable!(),
        }
    }
}

impl Display for InputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownFormat => write!(
                f,
                "Unknown escalator format, expected \"#-#,\" \"#/#,\" or \"all\""
            ),
            Self::InvalidFloor(floor) => write!(f, "{floor:?} is not a valid floor"),
            Self::InvalidEscalator(a, b) => write!(f, "The {a} to {b} is not an escalator"),
        }
    }
}

impl std::error::Error for InputError {}
