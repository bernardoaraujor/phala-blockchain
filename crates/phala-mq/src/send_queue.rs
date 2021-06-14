use crate::types::{SignedMessage, Message};
use crate::{Mutex, SenderId};
use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};

#[derive(Clone)]
pub struct MessageSendQueue {
    // Map: sender -> (sequence, messages)
    inner: Arc<Mutex<BTreeMap<SenderId, (u64, Vec<SignedMessage>)>>>,
}

impl MessageSendQueue {
    pub fn new() -> Self {
        MessageSendQueue {
            inner: Default::default(),
        }
    }
    pub fn create_handle<Si: Signer>(&self, sender: SenderId, signer: Si) -> MessageSendHandle<Si> {
        MessageSendHandle::new(self.clone(), sender, signer)
    }

    pub fn enqueue_message(
        &self,
        sender: SenderId,
        constructor: impl FnOnce(u64) -> SignedMessage,
    ) {
        let mut inner = self.inner.lock();
        let entry = inner.entry(sender).or_default();
        let message = constructor(entry.0);
        entry.1.push(message);
        entry.0 += 1;
    }

    pub fn all_messages(&self) -> Vec<SignedMessage> {
        let inner = self.inner.lock();
        inner
            .iter()
            .flat_map(|(_k, v)| v.1.iter().cloned())
            .collect()
    }

    pub fn messages(&self, sender: &SenderId) -> Vec<SignedMessage> {
        let inner = self.inner.lock();
        inner.get(sender).map(|x| x.1.clone()).unwrap_or(Vec::new())
    }

    /// Purge the messages which are aready accepted on chain.
    pub fn purge(&self, next_sequence_for: impl Fn(&SenderId) -> u64) {
        let mut inner = self.inner.lock();
        for (k, v) in inner.iter_mut() {
            let seq = next_sequence_for(k);
            v.1.retain(|msg| msg.sequence >= seq);
        }
    }
}

pub use msg_handle::*;
mod msg_handle {
    use super::*;
    use crate::{SenderId, types::Path};

    pub trait Signer {
        fn sign(&self, sequence: u64, message: &Message) -> Vec<u8>;
    }

    #[derive(Clone)]
    pub struct MessageSendHandle<Si: Signer> {
        queue: MessageSendQueue,
        sender: SenderId,
        signer: Si,
    }

    impl<Si: Signer> MessageSendHandle<Si> {
        pub fn new(queue: MessageSendQueue, sender: SenderId, signer: Si) -> Self {
            MessageSendHandle {
                queue,
                sender,
                signer,
            }
        }

        pub fn send(&self, payload: Vec<u8>, to: impl Into<Path>) {
            let sender = self.sender.clone();
            let signer = &self.signer;

            self.queue.enqueue_message(sender.clone(), move |sequence| {
                let message = Message {
                    sender,
                    destination: to.into(),
                    payload,
                };
                let signature = signer.sign(sequence, &message);
                SignedMessage { message, sequence, signature }
            })
        }
    }
}
