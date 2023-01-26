use crate::prelude::*;

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct MenuId {
    pub channel: serenity::ChannelId,
    pub message: serenity::MessageId,
}

#[derive(Clone)]
pub enum MenuUpdate {
    Create(MenuId, serenity::Message),
    Delete(MenuId),
}