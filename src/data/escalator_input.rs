use crate::prelude::*;

use super::ESCALATORS;
use std::{fmt::Display, str::FromStr};

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

    pub fn short_noun(&self) -> String {
        match self {
            Self::All => String::from("ALL escalators"),
            Self::Pair(a, b) => {
                let lower = a.min(b);
                let upper = a.max(b);
                format!("{lower}-{upper} and {upper}-{lower}")
            }
            Self::Direct(lower, upper) => format!("{lower}-{upper}"),
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
