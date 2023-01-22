use std::{any::{TypeId, Any}, collections::{HashMap, hash_map::Entry}};

use tokio::sync::broadcast;

pub struct AnyChannels {
    channels: HashMap<TypeId, Box<dyn Any + Send + Sync>>
}

impl AnyChannels {
    pub fn new() -> Self {
        AnyChannels { channels: HashMap::new() }
    }

    pub fn sender<T: 'static + Clone + Send + Sync>(&mut self) -> broadcast::Sender<T> {
        match self.channels.entry(TypeId::of::<T>()) {
            Entry::Occupied(entry) => {
                entry.get()
                    .downcast_ref::<broadcast::Sender<T>>()
                    .expect("The TypeId MUST map to a broadcast::Sender of the same type")
                    .clone()
            }
            Entry::Vacant(entry) => {
                let (tx, _rx) = broadcast::channel(16);
                entry.insert(Box::new(tx.clone()));

                tx
            }
        }
    }

    pub fn receiver<T: 'static + Clone + Send + Sync>(&mut self) -> broadcast::Receiver<T> {
        match self.channels.entry(TypeId::of::<T>()) {
            Entry::Occupied(entry) => {
                entry.get()
                    .downcast_ref::<broadcast::Sender<T>>()
                    .expect("The TypeId MUST map to a broadcast::Sender of the same type")
                    .subscribe()
            }
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
