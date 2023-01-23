pub use poise::serenity_prelude as serenity;

pub use crate::data::Data;

pub use crate::data::escalator::EscalatorFloors;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;

/// Creates a function that overwrites the original builder with a new given builder.
pub fn replace_builder_with<T>(builder: T) -> impl FnOnce(&mut T) -> &mut T {
    move |old_builder| {
        *old_builder = builder;
        old_builder
    }
}

pub fn is_valid_escalator(start: u8, end: u8) -> bool {
    // if the start and end are valid floors...
    ((2..=9).contains(&start) && (2..=9).contains(&end))
        // ...and the distance between the two floors is 2
        //                           ...or it is the 2-3 or 3-2
        && (start.abs_diff(end) == 2 || matches!((start, end), (2, 3) | (3, 2)))
}
