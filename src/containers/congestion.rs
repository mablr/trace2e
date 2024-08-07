use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use tokio::sync::{mpsc, oneshot, Mutex};

use crate::identifiers::Identifier;

#[derive(Debug, Default)]
pub struct QueuingHandler {
    queues: HashMap<Identifier, VecDeque<oneshot::Sender<()>>>,
}

pub enum QueuingMessage {
    EnterQueue(Identifier, oneshot::Sender<()>),
    ExitQueue(Identifier, oneshot::Sender<bool>),
}

impl QueuingHandler {
    pub fn enter_queue(
        &mut self,
        resource_identifier: Identifier,
        callback_channel: oneshot::Sender<()>,
    ) {
        let queue = self.queues.entry(resource_identifier.clone()).or_default();
        queue.push_back(callback_channel);
    }

    pub fn exit_queue(
        &mut self,
        resource_identifier: Identifier,
        responder: oneshot::Sender<bool>,
    ) {
        if let Some(callback_channel) = self
            .queues
            .get_mut(&resource_identifier)
            .and_then(|queue| queue.pop_front())
        {
            let _ = callback_channel.send(());
            let _ = responder.send(false);
        } else {
            let _ = responder.send(true);
        }
    }
}

pub async fn congestion_manager(
    mut receiver: mpsc::Receiver<QueuingMessage>,
    queuing_handler: Arc<Mutex<QueuingHandler>>,
) {
    while let Some(message) = receiver.recv().await {
        let mut queuing_handler = queuing_handler.lock().await;
        match message {
            QueuingMessage::EnterQueue(identifier, callback_channel) => {
                queuing_handler.enter_queue(identifier, callback_channel)
            }
            QueuingMessage::ExitQueue(identifier, responder) => {
                queuing_handler.exit_queue(identifier, responder)
            }
        }
    }
}
