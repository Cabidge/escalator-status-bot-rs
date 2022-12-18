pub mod commands;
pub mod prelude;

use poise::serenity_prelude as serenity;
use std::sync::Arc;

use prelude::*;

struct EscalatorBot {
    framework: Arc<poise::Framework<Data, Error>>,
}

pub struct Data;

#[shuttle_service::main]
async fn init(
    #[shuttle_secrets::Secrets] secret_store: shuttle_secrets::SecretStore,
) -> Result<EscalatorBot, shuttle_service::Error> {
    let Some(token) = secret_store.get("TOKEN") else {
        return Err(anyhow::anyhow!("Discord token not found...").into());
    };

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: commands::commands(),
            ..Default::default()
        })
        .token(token)
        .intents(serenity::GatewayIntents::non_privileged())
        .setup(|_ctx, _ready, _framework| {
            println!("Bot is ready");
            Box::pin(async move { Ok(Data) })
        })
        .build()
        .await
        .map_err(anyhow::Error::new)?;

    Ok(EscalatorBot { framework })
}

#[shuttle_service::async_trait]
impl shuttle_service::Service for EscalatorBot {
    async fn bind(
        mut self: Box<Self>,
        _addr: std::net::SocketAddr,
    ) -> Result<(), shuttle_service::error::Error> {
        self.framework.start().await.map_err(anyhow::Error::from)?;

        Ok(())
    }
}
