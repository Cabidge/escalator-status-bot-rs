pub mod bot_tasks;
pub mod commands;
pub mod data;
pub mod interaction;
pub mod prelude;
pub mod report_modal;

use std::sync::Arc;
use tokio::task;

use prelude::*;

struct EscalatorBot {
    framework: Arc<poise::Framework<Data, Error>>,
    save_task: task::JoinHandle<()>,
    announce_task: task::JoinHandle<()>,
    sync_task: task::JoinHandle<()>,
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

    let (data, updates_rx) = Data::load_persist(&persist);

    // create bot framework
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            // add bot commands
            commands: commands::commands(),
            event_handler: |ctx, event, _framework, data| {
                Box::pin(async move {
                    // handle component interactions
                    if let poise::Event::InteractionCreate { interaction } = event {
                        interaction::handle_interaction(ctx, interaction, data).await?;
                    }

                    Ok(())
                })
            },
            ..Default::default()
        })
        .token(token)
        .intents(serenity::GatewayIntents::non_privileged())
        .setup(move |_ctx, _ready, _framework| {
            // set up bot data
            println!("Bot is ready");
            Box::pin(async move { Ok(data) })
        })
        .build()
        .await
        .map_err(anyhow::Error::new)?;

    let save_task = bot_tasks::autosave::begin_task(Arc::clone(&framework), persist);
    let announce_task = bot_tasks::announcements::begin_task(Arc::clone(&framework), updates_rx.resubscribe());
    let sync_task = bot_tasks::sync_menu::begin_task(Arc::clone(&framework), updates_rx);

    Ok(EscalatorBot {
        framework,
        save_task,
        announce_task,
        sync_task,
    })
}

#[shuttle_service::async_trait]
impl shuttle_service::Service for EscalatorBot {
    async fn bind(
        mut self: Box<Self>,
        _addr: std::net::SocketAddr,
    ) -> Result<(), shuttle_service::error::Error> {
        self.framework.start().await.map_err(anyhow::Error::from)?;
        self.save_task.await.map_err(anyhow::Error::from)?;
        self.announce_task.await.map_err(anyhow::Error::from)?;
        self.sync_task.await.map_err(anyhow::Error::from)?;

        Ok(())
    }
}
