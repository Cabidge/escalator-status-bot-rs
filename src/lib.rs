pub mod bot_tasks;
pub mod commands;
pub mod data;
pub mod prelude;
pub mod report_modal;

use std::sync::Arc;
use tokio::{sync::{broadcast, mpsc}, task};

use prelude::*;

struct EscalatorBot {
    framework: Arc<poise::Framework<Data, Error>>,
    save_task: task::JoinHandle<()>,
    announce_task: task::JoinHandle<()>,
    sync_task: task::JoinHandle<()>,
    forward_report_task: task::JoinHandle<()>,
    check_outdated_task: task::JoinHandle<()>,
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
                let data = Data::load_persist(shard_manager, ctx, user_reports_tx, updates_tx, &persist).await;
                Ok(data)
            })
        })
        .build()
        .await
        .map_err(anyhow::Error::new)?;

    let save_task = bot_tasks::autosave::begin_task(Arc::clone(&framework), persist);
    let announce_task =
        bot_tasks::announcements::begin_task(Arc::clone(&framework), updates_rx.resubscribe());
    let sync_task = bot_tasks::sync_menu::begin_task(Arc::clone(&framework), updates_rx);
    let forward_report_task = bot_tasks::forward_reports::begin_task(Arc::clone(&framework), user_reports_rx);
    let check_outdated_task = bot_tasks::handle_outdated::begin_task(Arc::clone(&framework));

    Ok(EscalatorBot {
        framework,
        save_task,
        announce_task,
        sync_task,
        forward_report_task,
        check_outdated_task,
    })
}

#[shuttle_service::async_trait]
impl shuttle_service::Service for EscalatorBot {
    async fn bind(
        mut self: Box<Self>,
        _addr: std::net::SocketAddr,
    ) -> Result<(), shuttle_service::error::Error> {
        self.framework.start().await.map_err(anyhow::Error::from)?;

        // abort all bot tasks once client stops
        self.save_task.abort();
        self.announce_task.abort();
        self.sync_task.abort();
        self.forward_report_task.abort();
        self.check_outdated_task.abort();

        Ok(())
    }
}
