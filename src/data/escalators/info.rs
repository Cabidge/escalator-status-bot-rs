use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::Status;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Info {
    last_update: std::time::SystemTime,
    status: Option<Status>,
}

pub const UNKNOWN_STATUS_EMOJI: char = '🟡';

impl Info {
    /// How long it takes before marking an escalator's status as unknown (None).
    const OUTDATED_THRESHOLD: Duration = Duration::from_secs(2 * 60 * 60);

    pub fn status(&self) -> Option<Status> {
        self.status
    }

    pub fn status_emoji(&self) -> char {
        match self.status {
            Some(status) => status.emoji(),
            None => UNKNOWN_STATUS_EMOJI,
        }
    }

    /// Update the known status of this escalator, returning if it overrode the previous status.
    pub fn update_status(&mut self, status: Status) -> bool {
        self.last_update = std::time::SystemTime::now();
        let updated = self.status != Some(status);
        self.status = Some(status);

        updated
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
