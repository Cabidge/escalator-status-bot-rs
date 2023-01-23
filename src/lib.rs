mod bot_tasks;
mod commands;
mod data;
mod generate;
mod migration;
mod prelude;

use bot_tasks::*;
use shuttle_service::error::CustomError;
use std::sync::Arc;
use tokio::task;

use prelude::*;

struct EscalatorBot {
    framework: Arc<poise::Framework<Data, Error>>,
    tasks: Vec<task::JoinHandle<()>>,
}

#[shuttle_service::main]
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
            ..Default::default()
        })
        .token(token)
        .intents(serenity::GatewayIntents::non_privileged())
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                // set up bot data
                log::info!("Bot is ready");

                let shard_manager = Arc::clone(framework.shard_manager());

                migration::migrate_to_sqlx(&persist, &pool, &ctx).await;

                Ok(Data::new(shard_manager, pool))
            })
        })
        .build()
        .await
        .map_err(anyhow::Error::new)?;

    let bot = EscalatorBot::new(framework);

    Ok(bot)
}

impl EscalatorBot {
    fn new(framework: Arc<poise::Framework<Data, Error>>) -> Self {
        Self {
            framework,
            tasks: vec![],
        }
    }

    fn add_task<T: BotTask>(mut self, task: T) -> Self {
        self.tasks.push(task.begin(Arc::downgrade(&self.framework)));
        self
    }
}

#[shuttle_service::async_trait]
impl shuttle_service::Service for EscalatorBot {
    async fn bind(
        mut self: Box<Self>,
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
