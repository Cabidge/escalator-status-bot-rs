use std::{error::Error, fmt::Display, str::FromStr};

use serde::{Deserialize, Serialize};

#[derive(sqlx::Type, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[sqlx(type_name = "escalator_status", rename_all = "lowercase")]
pub enum Status {
    Open,
    Down,
    Blocked,
}

impl Status {
    pub const fn emoji(self) -> char {
        match self {
            Status::Open => '🟢',
            Status::Down => '🔴',
            Status::Blocked => '⛔',
        }
    }

    pub const fn as_id_str(&self) -> &'static str {
        match self {
            Status::Open => "OPEN",
            Status::Down => "DOWN",
            Status::Blocked => "BLOCKED",
        }
    }
}

#[derive(Debug)]
pub struct UnknownStatusError(String);

impl FromStr for Status {
    type Err = UnknownStatusError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let status = s.to_ascii_uppercase();
        match status.as_str() {
            "OPEN" => Ok(Self::Open),
            "DOWN" => Ok(Self::Down),
            "BLOCKED" => Ok(Self::Blocked),
            _ => Err(UnknownStatusError(status)),
        }
    }
}

impl Display for UnknownStatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Unknown status: {}", self.0)
    }
}

impl Error for UnknownStatusError {}
