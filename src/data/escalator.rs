use super::status::Status;

#[derive(sqlx::FromRow, Clone, Copy)]
pub struct Escalator {
    #[sqlx(flatten)]
    pub floors: EscalatorFloors,
    #[sqlx(rename = "current_status")]
    pub status: Status,
}

#[derive(sqlx::FromRow, Clone, Copy, PartialEq, Eq, Hash)]
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
