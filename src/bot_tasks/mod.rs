pub mod alert;
pub mod announce;
pub mod menus;

use crate::prelude::*;

use poise::serenity_prelude::CacheHttp;
use std::{future::Future, process::Termination, sync::Arc};

pub trait BotTask<T: CacheHttp>: Send + Sync {
    type Data: Send + 'static;
    type Term: Termination;

    async fn setup(&self, data: &Data, cache_http: Arc<T>) -> Option<Self::Data>;
    fn run(self, data: Self::Data) -> impl Future<Output = Self::Term> + Send;
}
