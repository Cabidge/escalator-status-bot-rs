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

    pub fn is_singular(&self) -> bool {
        matches!(self, Self::Direct(..))
    }
}
