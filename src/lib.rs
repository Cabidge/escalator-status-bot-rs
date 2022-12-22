mod bot_tasks;
mod commands;
mod data;
mod prelude;

use bot_tasks::*;
use std::sync::Arc;
use tokio::{
    sync::{broadcast, mpsc},
    task,
};

use prelude::*;

struct EscalatorBot {
    framework: Arc<poise::Framework<Data, Error>>,
    tasks: Vec<task::JoinHandle<()>>,
}

#[shuttle_service::main]
async fn init(
    #[shuttle_secrets::Secrets] secret_store: shuttle_secrets::SecretStore,
    #[shuttle_persist::Persist] persist: shuttle_persist::PersistInstance,
) -> Result<EscalatorBot, shuttle_service::Error> {
    // try to get token, errors if token isn't found
    let Some(token) = secret_store.get("TOKEN") else {
        return Err(anyhow::anyhow!("Discord token not found...").into());
    };

    let persist = Arc::new(persist);

    let (updates_tx, updates_rx) = broadcast::channel(32);
    let (user_reports_tx, user_reports_rx) = mpsc::channel(2);

    let cloned_persist = Arc::clone(&persist);

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
            // set up bot data
            let persist = cloned_persist;
            println!("Bot is ready");

            let shard_manager = Arc::clone(framework.shard_manager());

            Box::pin(async move {
                let data =
                    Data::load_persist(shard_manager, ctx, user_reports_tx, updates_tx, &persist)
                        .await;
                Ok(data)
            })
        })
        .build()
        .await
        .map_err(anyhow::Error::new)?;

    let bot = EscalatorBot::new(framework)
        .add_task(AutoSaveTask(persist))
        .add_task(AnnouncementTask(updates_rx.resubscribe()))
        .add_task(SyncMenuTask(updates_rx))
        .add_task(ForwardReportTask(user_reports_rx))
        .add_task(HandleOutdatedTask);

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
        self.tasks.push(task.begin(Arc::clone(&self.framework)));
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
