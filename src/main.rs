pub mod data;
pub mod generate;

mod bot_tasks;
mod commands;
mod migration;
mod prelude;

use bot_tasks::{
    alert::AlertTask,
    announce::AnnounceTask,
    menus::{info::InfoTask, report::ReportTask, sync::SyncTask},
    BotTask,
};
use futures::future::BoxFuture;
use poise::serenity_prelude::{MessageComponentInteraction, ShardMessenger};
use shuttle_runtime::async_trait;
use shuttle_service::error::CustomError;
use std::{process::Termination, sync::Arc};
use tokio::task;

use prelude::*;

struct EscalatorBot {
    framework: Arc<poise::Framework<Data, Error>>,
    tasks: Vec<task::JoinHandle<()>>,
}

#[shuttle_runtime::main]
async fn init(
    #[shuttle_secrets::Secrets] secret_store: shuttle_secrets::SecretStore,
    #[shuttle_persist::Persist] persist: shuttle_persist::PersistInstance,
    #[shuttle_shared_db::Postgres] pool: sqlx::PgPool,
) -> Result<EscalatorBot, shuttle_service::Error> {
    // try to get token, errors if token isn't found
    let Some(token) = secret_store.get("TOKEN") else {
        return Err(anyhow::anyhow!("Discord token not found...").into());
    };

    sqlx::migrate!()
        .run(&pool)
        .await
        .map_err(CustomError::new)?;

    // create bot framework
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            // add bot commands
            commands: commands::commands(),
            event_handler,
            ..Default::default()
        })
        .token(token)
        .intents(serenity::GatewayIntents::non_privileged())
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                // set up bot data
                log::info!("Bot is ready");

                let shard_manager = Arc::clone(framework.shard_manager());

                migration::migrate_to_sqlx(&persist, &pool, ctx).await;

                Ok(Data::new(shard_manager, pool))
            })
        })
        .build()
        .await
        .map_err(anyhow::Error::new)?;

    let bot = EscalatorBot::new(framework)
        .add_task(AnnounceTask::default())
        .add_task(AlertTask)
        .add_task(InfoTask)
        .add_task(ReportTask)
        .add_task(SyncTask);

    Ok(bot)
}

impl EscalatorBot {
    fn new(framework: Arc<poise::Framework<Data, Error>>) -> Self {
        Self {
            framework,
            tasks: vec![],
        }
    }

    fn add_task<T: BotTask + 'static>(mut self, task: T) -> Self {
        let framework = Arc::downgrade(&self.framework);
        let handle = tokio::spawn(async move {
            if let Some(data) = task.setup(framework).await {
                let code = task.run(data).await.report();
                log::debug!(
                    "Task {} failed with exit code: {:?}",
                    std::any::type_name::<T>(),
                    code
                )
            } else {
                log::error!(
                    "Faield to run setup for bot task: {}",
                    std::any::type_name::<T>()
                );
            }
        });

        self.tasks.push(handle);

        self
    }
}

#[async_trait]
impl shuttle_runtime::Service for EscalatorBot {
    async fn bind(
        mut self,
        _addr: std::net::SocketAddr,
    ) -> Result<(), shuttle_service::error::Error> {
        self.framework.start().await.map_err(anyhow::Error::from)?;

        // abort all bot tasks once client stops
        for task in self.tasks {
            task.abort();
        }

        Ok(())
    }
}

pub struct ComponentMessage {
    interaction: MessageComponentInteraction,
    shard: ShardMessenger,
}

/// TODO: rework this system and reduce cloning
fn event_handler<'a>(
    serenity_ctx: &'a serenity::Context,
    event: &'a poise::Event<'a>,
    ctx: poise::FrameworkContext<'a, Data, Error>,
    _data: &'a Data,
) -> BoxFuture<'a, Result<(), Error>> {
    use serenity::Interaction;

    if let poise::Event::InteractionCreate {
        interaction: Interaction::MessageComponent(interaction),
    } = event
    {
        if interaction.message.author.id == ctx.bot_id {
            let create_value = || {
                let message = ComponentMessage {
                    interaction: interaction.clone(),
                    shard: serenity_ctx.shard.clone(),
                };

                Arc::new(message)
            };

            ctx.user_data
                .send_message_with::<Arc<ComponentMessage>, _>(create_value);
        }
    }

    Box::pin(async move { Ok(()) })
}
