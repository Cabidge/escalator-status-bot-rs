use crate::{prelude::*, ui::view::View};

use super::MessageHandle;
use poise::{async_trait, CreateReply, ReplyHandle};

pub enum PoiseContextHandle<'a, const EPHEMERAL: bool> {
    Deferred {
        ctx: Context<'a>,
    },
    Sent {
        ctx: Context<'a>,
        reply: ReplyHandle<'a>,
    },
}

pub trait IntoPoiseContextHandle<'a> {
    fn into_handle<const EPHEMERAL: bool>(self) -> PoiseContextHandle<'a, EPHEMERAL>;
}

impl<'a> IntoPoiseContextHandle<'a> for Context<'a> {
    fn into_handle<const EPHEMERAL: bool>(self) -> PoiseContextHandle<'a, EPHEMERAL> {
        PoiseContextHandle::Deferred { ctx: self }
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
