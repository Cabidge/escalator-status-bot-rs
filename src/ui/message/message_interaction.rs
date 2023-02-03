use poise::{
    async_trait,
    serenity_prelude::{
        ComponentInteractionCollector, Http, MessageComponentInteraction, ShardMessenger,
    },
};

use crate::{prelude::*, ui::view::View};

use super::{MessageContext, MessageHandle};

pub struct MessageComponentHandle {
    interaction: MessageComponentInteraction,
    collector: ComponentInteractionCollector,
}

#[async_trait]
impl<'a> MessageContext<'a> for MessageComponentInteraction {
    type Handle = MessageComponentHandle;

    async fn send(
        self,
        view: View,
        ephemeral: bool,
        http: &Http,
        shard: &ShardMessenger,
    ) -> Result<Self::Handle, serenity::Error> {
        self.create_interaction_response(http, |res| {
            res.interaction_response_data(|data| {
                data.content(view.content)
                    .set_components(view.rows.into())
                    .ephemeral(ephemeral)
            })
        })
        .await?;

        let collector = self
            .get_interaction_response(http)
            .await?
            .await_component_interactions(shard)
            .build();

        Ok(MessageComponentHandle {
            interaction: self,
            collector,
        })
    }
}

#[async_trait]
impl MessageHandle for MessageComponentHandle {
    async fn edit(&mut self, view: View, http: &Http) -> Result<(), serenity::Error> {
        self.interaction
            .edit_original_interaction_response(http, |res| {
                res.content(view.content)
                    .components(replace_builder_with(view.rows.into()))
            })
            .await?;

        Ok(())
    }

    fn collector(&mut self) -> &mut ComponentInteractionCollector {
        &mut self.collector
    }
}
