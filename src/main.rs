pub mod data;
pub mod generate;

mod bot_tasks;
mod commands;
mod prelude;

use bot_tasks::{
    alert::AlertTask,
    announce::AnnounceTask,
    menus::{info::InfoTask, report::ReportTask, sync::SyncTask},
    BotTask,
};
use futures::future::BoxFuture;
use poise::serenity_prelude::{
    Cache, CacheHttp, ComponentInteraction, FullEvent, Http, ShardMessenger,
};
use shuttle_runtime::{async_trait, CustomError, SecretStore};
use std::{process::Termination, sync::Arc};
use tokio::task;

use prelude::*;

struct EscalatorBot {
    framework: poise::Framework<Data, Error>,
    data: Data,
    token: String,
}

struct BotTasks<T> {
    tasks: task::JoinSet<()>,
    data: Data,
    cache_http: Arc<T>,
}

struct CacheAndHttp(Arc<Cache>, Arc<Http>);

#[shuttle_runtime::main]
async fn init(
    #[shuttle_runtime::Secrets] secret_store: SecretStore,
    #[shuttle_shared_db::Postgres] pool: sqlx::PgPool,
) -> Result<EscalatorBot, shuttle_runtime::Error> {
    // try to get token, errors if token isn't found
    let Some(token) = secret_store.get("TOKEN") else {
        return Err(anyhow::anyhow!("Discord token not found...").into());
    };

    sqlx::migrate!()
        .run(&pool)
        .await
        .map_err(CustomError::new)?;

    let data = Data::new(pool);

    // create bot framework
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            // add bot commands
            commands: commands::commands(),
            event_handler,
            ..Default::default()
        })
        .setup({
            let data = data.clone();
            move |_ctx, _ready, _framework| {
                Box::pin(async move {
                    // set up bot data
                    log::info!("Bot is ready");
                    Ok(data)
                })
            }
        })
        .build();

    let bot = EscalatorBot::new(framework, data, token);
    Ok(bot)
}

impl EscalatorBot {
    fn new(framework: poise::Framework<Data, Error>, data: Data, token: String) -> Self {
        Self {
            framework,
            data,
            token,
        }
    }
}

#[async_trait]
impl shuttle_runtime::Service for EscalatorBot {
    async fn bind(mut self, _addr: std::net::SocketAddr) -> Result<(), shuttle_runtime::Error> {
        let intents = serenity::GatewayIntents::non_privileged();

        let mut client = serenity::ClientBuilder::new(self.token, intents)
            .framework(self.framework)
            .await
            .unwrap();

        let cache = Arc::clone(&client.cache);
        let http = Arc::clone(&client.http);
        let cache_http = Arc::new(CacheAndHttp(cache, http));
        let mut bot_tasks = BotTasks::new(self.data, cache_http)
            .start_task(AnnounceTask::default())
            .await?
            .start_task(AlertTask)
            .await?
            .start_task(InfoTask)
            .await?
            .start_task(ReportTask)
            .await?
            .start_task(SyncTask)
            .await?;

        client.start().await.map_err(anyhow::Error::from)?;

        // abort all bot tasks once client stops
        bot_tasks.tasks.abort_all();

        Ok(())
    }
}

impl CacheHttp for CacheAndHttp {
    fn cache(&self) -> Option<&Arc<Cache>> {
        Some(&self.0)
    }

    fn http(&self) -> &Http {
        &self.1
    }
}

impl<T: CacheHttp> BotTasks<T> {
    fn new(data: Data, cache_http: Arc<T>) -> Self {
        Self {
            tasks: task::JoinSet::new(),
            data,
            cache_http,
        }
    }

    async fn start_task(mut self, task: impl BotTask<T> + 'static) -> anyhow::Result<Self> {
        let Some(data) = task.setup(&self.data, Arc::clone(&self.cache_http)).await else {
            anyhow::bail!(
                "Faield to run setup for bot task: {}",
                std::any::type_name::<T>()
            );
        };

        self.tasks.spawn(async move {
            let code = task.run(data).await.report();
            log::debug!(
                "Task {} failed with exit code: {:?}",
                std::any::type_name::<T>(),
                code
            )
        });

        Ok(self)
    }
}

pub struct ComponentMessage {
    interaction: ComponentInteraction,
    shard: ShardMessenger,
}

/// TODO: rework this system and reduce cloning
fn event_handler<'a>(
    serenity_ctx: &'a serenity::Context,
    event: &'a FullEvent,
    ctx: poise::FrameworkContext<'a, Data, Error>,
    _data: &'a Data,
) -> BoxFuture<'a, Result<(), Error>> {
    use serenity::Interaction;

    log::debug!("Event received: {event:?}");

    let FullEvent::InteractionCreate { interaction } = event else {
        return Box::pin(async { Ok(()) });
    };

    if let Interaction::Component(interaction) = interaction {
        if interaction.message.author.id == ctx.bot_id {
            log::debug!("Interaction received: {interaction:?}");

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
