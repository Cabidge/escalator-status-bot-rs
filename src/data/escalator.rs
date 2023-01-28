use std::fmt::Display;

use super::status::Status;

#[derive(sqlx::FromRow, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Escalator {
    #[sqlx(flatten)]
    pub floors: EscalatorFloors,
    #[sqlx(rename = "current_status")]
    pub status: Status,
}

#[derive(sqlx::FromRow, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EscalatorFloors {
    #[sqlx(rename = "floor_start", try_from = "i16")]
    pub start: u8,
    #[sqlx(rename = "floor_end", try_from = "i16")]
    pub end: u8,
}

impl EscalatorFloors {
    pub fn new(start: u8, end: u8) -> Self {
        Self { start, end }
    }
}

impl Display for Escalator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.status.emoji(), self.floors)
    }
}

impl Display for EscalatorFloors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.start, self.end)
    }
}
