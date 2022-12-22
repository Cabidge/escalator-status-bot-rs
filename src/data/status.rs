use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
