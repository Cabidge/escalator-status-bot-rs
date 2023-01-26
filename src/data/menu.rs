use crate::prelude::*;

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct MenuId {
    pub channel: serenity::ChannelId,
    pub message: serenity::MessageId,
}

pub enum MenuUpdate {
    Create(MenuId, serenity::Message),
    Delete(MenuId),
}