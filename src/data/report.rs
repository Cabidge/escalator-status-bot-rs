use crate::prelude::*;

use super::{escalator_input::EscalatorInput, status::Status};

use smallvec::SmallVec;

#[derive(Clone)]
pub struct UserReport {
    reporter: Option<serenity::UserId>,
    escalators: EscalatorInput,
    affected_escalators: SmallVec<[EscalatorFloors; 2]>,
    new_status: Status,
}
