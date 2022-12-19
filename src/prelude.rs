pub use poise::serenity_prelude as serenity;

pub use crate::data::Data;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;

/// Creates a function that overwrites the original builder with a new given builder.
pub fn replace_builder_with<T>(builder: T) -> impl FnOnce(&mut T) -> &mut T {
    move |old_builder| {
        *old_builder = builder;
        old_builder
    }
}