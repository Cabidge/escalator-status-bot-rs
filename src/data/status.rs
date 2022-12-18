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
}
