use crate::{prelude::*, bot_tasks::BotTask};

use poise::async_trait;

pub struct SyncTask;

pub struct TaskData {
}

#[async_trait]
impl BotTask for SyncTask {
    type Data = TaskData;
    type Term = anyhow::Result<()>;

    async fn setup(
        &self,
        framework: std::sync::Weak<poise::Framework<Data, Error>>,
    ) -> Option<Self::Data> {
        todo!()
    }

    async fn run(self, mut data: Self::Data) -> Self::Term {
        todo!()
    }
}
