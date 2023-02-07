use poise::{
    async_trait,
    serenity_prelude::{Http, MessageComponentInteraction},
};

use crate::{prelude::*, ui::view::View};

use super::MessageHandle;

pub struct InteractionHandle<'a, const EPHEMERAL: bool> {
    pub interaction: &'a MessageComponentInteraction,
    http: &'a Http,
    sent: bool,
}

pub trait ToInteractionHandle {
    fn to_handle<'a, const EPHEMERAL: bool>(&'a self, http: &'a Http) -> InteractionHandle<'a, EPHEMERAL>;
}

impl ToInteractionHandle for MessageComponentInteraction {
    fn to_handle<'a, const EPHEMERAL: bool>(&'a self, http: &'a Http) -> InteractionHandle<'a, EPHEMERAL> {
        InteractionHandle {
            interaction: self,
            http,
            sent: false,
        }
    }
}

#[async_trait]
impl<'a, const EPHEMERAL: bool> MessageHandle for InteractionHandle<'a, EPHEMERAL> {
    async fn show(
        &mut self,
        view: View,
    ) -> Result<(), serenity::Error> {
        if self.sent {
            self.interaction.edit_original_interaction_response(self.http, |res| {
                res.content(view.content)
                    .components(replace_builder_with(view.rows.into()))
            })
            .await?;
        } else {
            self.interaction.create_interaction_response(self.http, |res| {
                res.interaction_response_data(|data| {
                    data.content(view.content)
                        .set_components(view.rows.into())
                        .ephemeral(EPHEMERAL)
                })
            })
            .await?;

            self.sent = true;
        }

        Ok(())
    }

    async fn get_message(&self) -> Option<serenity::Message> {
        self.interaction.get_interaction_response(self.http).await.ok()
    }
}
