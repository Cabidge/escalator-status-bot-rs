use std::{
    any::{Any, TypeId},
    collections::{hash_map::Entry, HashMap},
};

use tokio::sync::broadcast;

pub struct AnyChannels {
    channels: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl AnyChannels {
    pub fn new() -> Self {
        AnyChannels {
            channels: HashMap::new(),
        }
    }

    /// Tries to send out a given value,
    /// but doesn't generate a Sender if one doesn't exist already.
    pub fn try_send<T: 'static + Clone + Send + Sync>(
        &self,
        value: T,
    ) -> Result<usize, broadcast::error::SendError<T>> {
        match self.try_sender() {
            Some(sender) => sender.send(value),
            None => Err(broadcast::error::SendError(value)),
        }
    }

    /// Tries to obtain a sender reference, returns None if not exists.
    pub fn try_sender<T: 'static + Clone + Send + Sync>(&self) -> Option<&broadcast::Sender<T>> {
        self.channels.get(&TypeId::of::<T>()).map(|any| {
            any.downcast_ref()
                .expect("The TypeId MUST map to a broadcast::Sender of the same type")
        })
    }

    pub fn sender<T: 'static + Clone + Send + Sync>(&mut self) -> broadcast::Sender<T> {
        match self.channels.entry(TypeId::of::<T>()) {
            Entry::Occupied(entry) => entry
                .get()
                .downcast_ref::<broadcast::Sender<T>>()
                .expect("The TypeId MUST map to a broadcast::Sender of the same type")
                .clone(),
            Entry::Vacant(entry) => {
                let (tx, _rx) = broadcast::channel(16);
                entry.insert(Box::new(tx.clone()));

                tx
            }
        }
    }

    pub fn receiver<T: 'static + Clone + Send + Sync>(&mut self) -> broadcast::Receiver<T> {
        match self.channels.entry(TypeId::of::<T>()) {
            Entry::Occupied(entry) => entry
                .get()
                .downcast_ref::<broadcast::Sender<T>>()
                .expect("The TypeId MUST map to a broadcast::Sender of the same type")
                .subscribe(),
            Entry::Vacant(entry) => {
                let (tx, rx) = broadcast::channel(16);
                entry.insert(Box::new(tx));

                rx
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_create_sender_then_receiver() {
        let mut channels = AnyChannels::new();

        let tx = channels.sender::<i32>();
        let _rx = channels.receiver::<i32>();

        assert_eq!(tx.receiver_count(), 1);
    }

    #[test]
    fn can_create_receiver_then_sender() {
        let mut channels = AnyChannels::new();

        let _rx = channels.receiver::<i32>();
        let tx = channels.sender::<i32>();

        assert_eq!(tx.receiver_count(), 1);
    }

    #[test]
    fn messages_are_passed() {
        let mut channels = AnyChannels::new();

        let tx = channels.sender::<i32>();
        let mut rx = channels.receiver::<i32>();

        assert_eq!(rx.len(), 0);

        assert_eq!(tx.send(100).unwrap(), 1);
        assert_eq!(rx.len(), 1);

        assert_eq!(rx.try_recv().unwrap(), 100);
    }
}
