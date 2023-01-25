use crate::prelude::*;

use super::{escalator_input::EscalatorInput, status::Status};

use smallvec::SmallVec;
use std::fmt::Display;

#[derive(Clone)]
pub struct UserReport {
    reporter: Option<serenity::UserId>,
    escalators: EscalatorInput,
    affected_escalators: SmallVec<[EscalatorFloors; 2]>,
    new_status: Status,
}

impl Display for UserReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let emoji = self.new_status.emoji();
        let reporter = self
            .reporter
            .map(|id| format!("<@{}>", id))
            .unwrap_or_else(|| String::from("an unknown user"));

        write!(
            f,
            "`{emoji}` {reporter} reported {}.",
            self.escalators.message_noun()
        )
    }
}
