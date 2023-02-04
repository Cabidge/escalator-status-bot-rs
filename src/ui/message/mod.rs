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
use std::{future, sync::Arc};
use tokio::sync::mpsc;

pub struct MessageInterface<H> {
    pub handle: H,
    pub http: Arc<Http>,
    pub shard: ShardMessenger,
}

#[async_trait]
pub trait MessageContext: Sized + Send {
    type Handle: MessageHandle;

    async fn send(
        self,
        view: View,
        ephemeral: bool,
        http: &Http,
    ) -> Result<Self::Handle, serenity::Error>;

    async fn bind(self, ephemeral: bool, http: Arc<Http>, shard: ShardMessenger) -> Result<MessageInterface<Self::Handle>, serenity::Error> {
        let handle = self.send(ViewBuilder::with_content("*initializing...*").build(), ephemeral, &http).await?;

        Ok(MessageInterface {
            handle,
            http,
            shard,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Conclusion {
    Halt,
    Timeout,
}

#[async_trait]
pub trait MessageHandle: Send + Sync {
    async fn edit(&self, view: View, http: &Http) -> Result<(), serenity::Error>;
    async fn message(&self, http: &Http) -> Result<serenity::Message, serenity::Error>;
}

#[async_trait]
impl<T: MessageHandle> UserInterface for MessageInterface<T> {
    async fn run<C: Component>(
        &self,
        mut component: C,
        config: UiConfig,
        mut signals: mpsc::UnboundedReceiver<Signal<C>>,
    ) -> UiResult<C> {
        let http = &self.http;

        let show = |view: ViewBuilder| async {
            self.handle
                .edit(view.build(), http)
                .await
                .map_err(CustomError::new)
        };

        let mut timeout = config.sleeper();

        let mut collector = self.handle
            .message(http)
            .await
            .map_err(CustomError::new)?
            .await_component_interactions(&self.shard)
            .build();

        let mut view = if let Some(sleeper) = &mut timeout {
            ViewBuilder::with_timeout(sleeper)
        } else {
            ViewBuilder::default()
        };

        component.render(&mut view);

        show(view).await?;

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

                    show(view).await?;
                }
            }
        };

        if conclusion == Conclusion::Timeout {
            self.handle
                .edit(ViewBuilder::with_content("*timed out*").build(), http)
                .await
                .map_err(CustomError::new)?;
            return Err(UiError::Timeout);
        }

        let Some(output) = component.conclude() else {
            self.handle.edit(ViewBuilder::with_content("*interaction failed to complete*").build(), http).await.map_err(CustomError::new)?;
            return Err(UiError::Incomplete);
        };

        Ok(output)
    }
}
