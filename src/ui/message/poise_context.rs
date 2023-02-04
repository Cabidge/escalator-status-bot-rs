use crate::{prelude::*, ui::view::View};

use super::{MessageContext, MessageHandle};
use poise::{async_trait, serenity_prelude::Http, CreateReply, ReplyHandle};

pub struct PoiseContextHandle<'a> {
    ctx: Context<'a>,
    reply: ReplyHandle<'a>,
}

#[async_trait]
impl<'a> MessageContext<'a> for Context<'a> {
    type Handle = PoiseContextHandle<'a>;

    async fn send(
        self,
        view: View,
        ephemeral: bool,
        _http: &Http,
    ) -> Result<Self::Handle, serenity::Error> {
        let reply = self
            .send(|reply| create_view_reply(reply, view).ephemeral(ephemeral))
            .await?;

        Ok(PoiseContextHandle { ctx: self, reply })
    }
}

#[async_trait]
impl<'a> MessageHandle for PoiseContextHandle<'a> {
    async fn edit(&mut self, view: View, _http: &Http) -> Result<(), serenity::Error> {
        self.reply
            .edit(self.ctx, |reply| create_view_reply(reply, view))
            .await
    }

    async fn message(&mut self, _http: &Http) -> Result<serenity::Message, serenity::Error> {
        self.reply.message().await.map(|msg| msg.into_owned())
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
