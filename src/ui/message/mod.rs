pub mod message_interaction;
pub mod poise_context;

use crate::prelude::*;

use super::{
    view::View, Component, CustomError, Signal, UiConfig, UiError, UiResult, Update, UserInterface,
    ViewBuilder,
};

use futures::StreamExt;
use poise::{
    async_trait,
    serenity_prelude::{Http, ShardMessenger},
};
use std::future;
use tokio::sync::mpsc;

pub struct MessageInterface<'a, H> {
    pub handle: H,
    pub http: &'a Http,
    pub shard: &'a ShardMessenger,
}

#[async_trait]
pub trait MessageHandle: Sized + Send + Sync {
    async fn show(
        &mut self,
        view: View,
    ) -> Result<(), serenity::Error>;

    async fn get_message(&self) -> Option<serenity::Message>;

    fn create_ui<'a>(
        self,
        http: &'a Http,
        shard: &'a ShardMessenger,
    ) -> MessageInterface<'a, Self> {
        MessageInterface {
            handle: self,
            http,
            shard,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Conclusion {
    Halt,
    Timeout,
}

#[async_trait]
impl<'a, T: MessageHandle> UserInterface for MessageInterface<'a, T> {
    async fn run<C: Component>(
        &mut self,
        mut component: C,
        config: UiConfig,
        mut signals: mpsc::UnboundedReceiver<Signal<C>>,
    ) -> UiResult<C> {
        let http = &self.http;

        async fn show(handle: &mut impl MessageHandle, view: ViewBuilder) -> Result<(), CustomError> {
            handle
                .show(view.build())
                .await
                .map_err(CustomError::new)
        }

        let mut timeout = config.sleeper();

        let mut view = if let Some(sleeper) = &mut timeout {
            ViewBuilder::with_timeout(sleeper)
        } else {
            ViewBuilder::default()
        };

        component.render(&mut view);

        show(&mut self.handle, view).await?;

        let mut collector = self
            .handle
            .get_message()
            .await
            .ok_or_else(|| CustomError(anyhow::anyhow!("Message not found...")))?
            .await_component_interactions(self.shard)
            .build();

        let conclusion = loop {
            let wait_for_timeout = async {
                let Some(sleeper) = &mut timeout else {
                    future::pending::<()>().await;
                    unreachable!()
                };

                sleeper.await;
            };

            let signal = tokio::select! {
                collected = collector.next() => {
                    let Some(interaction) = collected else {
                        break Conclusion::Halt;
                    };

                    interaction.defer(&http).await.map_err(CustomError::new)?;

                    let Ok(action) = interaction.data.custom_id.parse::<C::Action>()  else {
                        log::warn!("An error ocurred parsing a component command");
                        continue;
                    };

                    Signal::Action(action)
                },
                Some(signal) = signals.recv() => signal,
                _ = wait_for_timeout => break Conclusion::Timeout,
            };

            let update = match signal {
                Signal::Action(action) => {
                    let Some(signal) = component.update(action) else { continue };
                    signal
                }
                Signal::Halt => Update::Halt,
            };

            match update {
                Update::Halt => {
                    break Conclusion::Halt;
                }
                Update::Render => {
                    let mut view = if let Some(sleeper) = &mut timeout {
                        sleeper.notify();
                        ViewBuilder::with_timeout(sleeper)
                    } else {
                        ViewBuilder::default()
                    };

                    component.render(&mut view);

                    show(&mut self.handle, view).await?;
                }
            }
        };

        if conclusion == Conclusion::Timeout {
            show(&mut self.handle, ViewBuilder::with_content("*timed out*"))
                .await?;

            return Err(UiError::Timeout);
        }

        if let Some(output) = component.conclude() {
            return Ok(output);
        };

        show(&mut self.handle, ViewBuilder::with_content("*interaction failed to complete*"))
            .await?;

        Err(UiError::Incomplete)
    }
}
