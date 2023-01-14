use crate::{data::ESCALATORS, prelude::*};

pub enum Iter {
    All(usize),
    Pair(u8, u8),
    Direct(u8, u8, bool),
}

impl Iterator for Iter {
    type Item = Escalator;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::All(i) => {
                let escalator = ESCALATORS.get(*i)?;
                *i += 1;

                Some(*escalator)
            }
            &mut Self::Pair(start, end) => {
                let escalator = (start, end);

                *self = Self::Direct(end, start, false);

                Some(escalator)
            }
            Self::Direct(start, end, done @ false) => {
                *done = true;
                Some((*start, *end))
            },
            Self::Direct(_, _, true) => None,
        }
    }
}
