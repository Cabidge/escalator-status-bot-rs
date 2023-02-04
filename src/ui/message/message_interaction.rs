use poise::{
    async_trait,
    serenity_prelude::{Http, MessageComponentInteraction},
};

use crate::{prelude::*, ui::view::View};

use super::{MessageContext, MessageHandle};

#[async_trait]
impl<'a> MessageContext<'a> for MessageComponentInteraction {
    type Handle = Self;

    async fn send(
        self,
        view: View,
        ephemeral: bool,
        http: &Http,
    ) -> Result<Self::Handle, serenity::Error> {
        self.create_interaction_response(http, |res| {
            res.interaction_response_data(|data| {
                data.content(view.content)
                    .set_components(view.rows.into())
                    .ephemeral(ephemeral)
            })
        })
        .await?;

        Ok(self)
    }
}

#[async_trait]
impl MessageHandle for MessageComponentInteraction {
    async fn edit(&mut self, view: View, http: &Http) -> Result<(), serenity::Error> {
        self.edit_original_interaction_response(http, |res| {
            res.content(view.content)
                .components(replace_builder_with(view.rows.into()))
        })
        .await?;

        Ok(())
    }

    async fn message(&mut self, http: &Http) -> Result<serenity::Message, serenity::Error> {
        self.get_interaction_response(http).await
    }
}
