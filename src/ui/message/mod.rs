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

pub struct MessageInterface<Ctx> {
    ctx: Ctx,
    http: Arc<Http>,
    shard: ShardMessenger,
}

#[async_trait]
pub trait MessageContext<'a>: Sized + Send {
    type Handle: MessageHandle + 'a;

    async fn send(
        self,
        view: View,
        ephemeral: bool,
        http: &Http,
    ) -> Result<Self::Handle, serenity::Error>;

    fn bind(self, http: Arc<Http>, shard: ShardMessenger) -> MessageInterface<Self> {
        MessageInterface {
            ctx: self,
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
pub trait MessageHandle: Send + Sync {
    async fn edit(&self, view: View, http: &Http) -> Result<(), serenity::Error>;
    async fn message(&self, http: &Http) -> Result<serenity::Message, serenity::Error>;
}

#[async_trait]
impl<T: MessageContext<'a> + 'a, 'a> UserInterface<'a> for MessageInterface<T> {
    async fn run<C: Component>(
        self,
        mut component: C,
        config: UiConfig,
        mut signals: mpsc::UnboundedReceiver<Signal<C>>,
    ) -> UiResult<C> {
        let http = &self.http;

        let mut timeout = config.sleeper();

        let handle = {
            let mut view = if let Some(sleeper) = &timeout {
                ViewBuilder::with_timeout(sleeper)
            } else {
                ViewBuilder::default()
            };

            component.render(&mut view);

            self.ctx
                .send(view.build(), config.ephemeral, http)
                .await
                .map_err(CustomError::new)?
        };

        let mut collector = handle
            .message(http)
            .await
            .map_err(CustomError::new)?
            .await_component_interactions(&self.shard)
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

                    handle
                        .edit(view.build(), http)
                        .await
                        .map_err(CustomError::new)?;
                }
            }
        };

        if conclusion == Conclusion::Timeout {
            handle
                .edit(ViewBuilder::with_content("*timed out*").build(), http)
                .await
                .map_err(CustomError::new)?;
            return Err(UiError::Timeout);
        }

        let Some(output) = component.conclude() else {
            handle.edit(ViewBuilder::with_content("*interaction failed to complete*").build(), http).await.map_err(CustomError::new)?;
            return Err(UiError::Incomplete);
        };

        Ok(output)
    }
}
