use std::{cmp::Ordering, time::Duration};

use serde::{Deserialize, Serialize};

use super::Status;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Info {
    last_update: std::time::SystemTime,
    status: Option<Status>,
}

pub const UNKNOWN_STATUS_EMOJI: char = '🟡';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportKind {
    Normal,
    Redundant,
    /// If report is updating an unknown status
    Rejuvenate,
}

impl Info {
    /// How long it takes before marking an escalator's status as unknown (None).
    const OUTDATED_THRESHOLD: Duration = Duration::from_secs((5 * 24 + 1) * 60 * 60);

    pub fn status(&self) -> Option<Status> {
        self.status
    }

    pub fn status_emoji(&self) -> char {
        match self.status {
            Some(status) => status.emoji(),
            None => UNKNOWN_STATUS_EMOJI,
        }
    }

    /// Update the known status of this escalator, returning what kind of report it is.
    pub fn update_status(&mut self, status: Status) -> ReportKind {
        self.last_update = std::time::SystemTime::now();

        let previous = self.status;

        self.status = Some(status);

        match (previous, status) {
            (Some(old), new) if old == new => ReportKind::Redundant,
            (None, _) => ReportKind::Rejuvenate,
            _ => ReportKind::Normal,
        }
    }

    /// Checks if the last time the escalator was updated is beyond a given threshold,
    /// setting the status to None if it is.
    pub fn handle_outdated(&mut self) -> bool {
        if self.status.is_some() && self.is_outdated() {
            self.status = None;
            true
        } else {
            false
        }
    }

    pub fn is_down(&self) -> bool {
        self.status == Some(Status::Down)
    }

    pub fn is_blocked(&self) -> bool {
        self.status == Some(Status::Blocked)
    }

    pub fn is_out_of_order(&self) -> bool {
        self.is_down() || self.is_blocked()
    }

    fn is_outdated(&self) -> bool {
        self.status.is_none()
            || std::time::SystemTime::now()
                .duration_since(self.last_update)
                .expect("Time has reversed")
                >= Self::OUTDATED_THRESHOLD
    }
}

impl Default for Info {
    fn default() -> Self {
        Self {
            last_update: std::time::UNIX_EPOCH,
            status: None,
        }
    }
}

impl PartialOrd for ReportKind {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ReportKind {
    fn cmp(&self, other: &Self) -> Ordering {
        match (*self, *other) {
            (this, that) if this == that => Ordering::Equal,
            (Self::Normal, _) | (_, Self::Redundant) => Ordering::Greater,
            (_, Self::Normal) | (Self::Redundant, _) => Ordering::Less,
            (Self::Rejuvenate, Self::Rejuvenate) => unreachable!("If self and other are both rejuvenate, the first branch would've already caught it"),
        }
    }
}
