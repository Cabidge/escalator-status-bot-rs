use crate::{prelude::*, ui::view::View};

use super::{MessageHandle, MessageInterface};
use poise::{async_trait, CreateReply, ReplyHandle, serenity_prelude::CacheHttp};

pub enum PoiseContextHandle<'a, const EPHEMERAL: bool> {
    Deferred {
        ctx: Context<'a>,
    },
    Sent {
        ctx: Context<'a>,
        reply: ReplyHandle<'a>,
    },
}

pub trait PoiseContextHandleExt<'a> {
    fn as_handle<const EPHEMERAL: bool>(&'a self) -> PoiseContextHandle<'a, EPHEMERAL>;
    fn as_ui<const EPHEMERAL: bool>(&'a self) -> MessageInterface<'a, PoiseContextHandle<'a, EPHEMERAL>>;
}

impl<'a> PoiseContextHandleExt<'a> for Context<'a> {
    fn as_handle<const EPHEMERAL: bool>(&'a self) -> PoiseContextHandle<'a, EPHEMERAL> {
        PoiseContextHandle::Deferred { ctx: *self }
    }

    fn as_ui<const EPHEMERAL: bool>(&'a self) -> MessageInterface<'a, PoiseContextHandle<'a, EPHEMERAL>> {
        self.as_handle().into_ui(self.http(), &self.serenity_context().shard)
    }
}

#[async_trait]
impl<'a, const EPHEMERAL: bool> MessageHandle for PoiseContextHandle<'a, EPHEMERAL> {
    async fn show(&mut self, view: View) -> Result<(), serenity::Error> {
        match self {
            &mut Self::Deferred { ctx } => {
                let reply = ctx
                    .send(|reply| create_view_reply(reply, view).ephemeral(EPHEMERAL))
                    .await?;

                *self = Self::Sent { ctx, reply }
            }
            Self::Sent { ctx, reply } => {
                reply
                    .edit(*ctx, |reply| create_view_reply(reply, view))
                    .await?;
            }
        }

        Ok(())
    }

    async fn get_message(&self) -> Option<serenity::Message> {
        match self {
            Self::Deferred { .. } => None,
            Self::Sent { reply, .. } => reply.message().await.map(|msg| msg.into_owned()).ok(),
        }
    }
}

fn create_view_reply<'a, 'att>(
    reply: &'a mut CreateReply<'att>,
    view: View,
) -> &'a mut CreateReply<'att> {
    reply
        .content(view.content)
        .components(replace_builder_with(view.rows.into()))
}
