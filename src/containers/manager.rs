use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};
use tokio::sync::{mpsc, oneshot, Mutex};

use crate::identifiers::Identifier;

use super::Container;

/// Global management structure for [`Container`] instances.
///
/// It offers a reliable and safe interface to acquire reservation in order to manipulate
/// `Containers`.
#[derive(Debug, Default)]
pub struct ContainersManager {
    containers: HashMap<Identifier, Container>,
}

impl ContainersManager {
    /// This method checks the presence of the provided key before instantiating
    /// and inserting a new [`Container`] to avoid overwriting an existing [`Container`]
    ///
    /// This will return `true` if a new [`Container`] has been instantiated and inserted,
    /// and `false` if a [`Container`] already exists for the provided key.
    pub fn register(&mut self, resource_identifier: Identifier) -> bool {
        if self.containers.contains_key(&resource_identifier) == false {
            let container = Container::new();
            self.containers.insert(resource_identifier, container);
            true
        } else {
            false
        }
    }

    /// Reserve the [`Container`] registered with the provided key.
    ///
    /// If the [`Container`] is available, it is set as reserved and `Ok(true)` is
    /// returned, if the [`Container`] is already reserved `Ok(false)` is returned.
    ///
    /// # Errors
    /// If there is no [`Container`] registered with the provided key an error is returned.
    pub fn try_reservation(&mut self, resource_identifier: Identifier) -> Result<bool, String> {
        if let Some(container) = self.containers.get_mut(&resource_identifier) {
            if container.is_available() {
                container.set_availability(false);
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            // Todo: Create specific error type
            Err(format!(
                "Container '{}' is not registered, impossible to reserve it.",
                resource_identifier
            ))
        }
    }

    /// Release the [`Container`] registered with the provided key.
    ///
    /// If the [`Container`] is reserved, it is set as available and `Ok(())` is
    /// returned
    ///
    /// # Errors
    /// If the [`Container`] is already available an error is returned.
    ///
    /// If there is no [`Container`] registered with the provided key an error is returned.
    pub fn try_release(&mut self, resource_identifier: Identifier) -> Result<(), String> {
        if let Some(container) = self.containers.get_mut(&resource_identifier) {
            if container.is_available() == false {
                container.set_availability(true);
                Ok(())
            } else {
                // Todo: Create specific error type
                Err(format!(
                    "Container '{}' is not reserved, impossible to release it.",
                    resource_identifier
                ))
            }
        } else {
            // Todo: Create specific error type
            Err(format!(
                "Container '{}' is not registered, impossible to release it.",
                resource_identifier
            ))
        }
    }
}

pub enum ContainerAction {
    Register(Identifier, oneshot::Sender<ContainerResult>),
    Reserve(Identifier, oneshot::Sender<ContainerResult>),
    Release(Identifier, oneshot::Sender<ContainerResult>),
}

#[derive(Debug)]
pub enum ContainerResult {
    Done,
    Wait(oneshot::Receiver<()>),
    Error,
}

pub async fn containers_manager(mut receiver: mpsc::Receiver<ContainerAction>) {
    let queues = Arc::new(Mutex::new(HashMap::new()));
    let manager = Arc::new(Mutex::new(ContainersManager::default()));
    while let Some(message) = receiver.recv().await {
        match message {
            ContainerAction::Register(identifier, responder) => {
                manager.lock().await.register(identifier);
                responder.send(ContainerResult::Done).unwrap();
            }
            ContainerAction::Reserve(identifier, responder) => {
                match manager.lock().await.try_reservation(identifier.clone()) {
                    Ok(true) => {
                        responder.send(ContainerResult::Done).unwrap();
                    }
                    Ok(false) => {
                        let callback =
                            wait_container_release(queues.clone(), identifier.clone()).await;
                        responder.send(ContainerResult::Wait(callback)).unwrap();
                    }
                    Err(_) => {
                        responder.send(ContainerResult::Error).unwrap(); // Todo create error
                    }
                };
            }
            ContainerAction::Release(identifier, responder) => {
                if let Some(channel) = queues
                    .lock()
                    .await
                    .get_mut(&identifier)
                    .and_then(|queue| queue.pop_front())
                {
                    channel.send(()).unwrap();
                    responder.send(ContainerResult::Done).unwrap();
                } else {
                    match manager.lock().await.try_release(identifier.clone()) {
                        Ok(()) => responder.send(ContainerResult::Done).unwrap(),
                        Err(_) => responder.send(ContainerResult::Error).unwrap(),
                    }
                }
            }
        }
    }
}

async fn wait_container_release(
    queues: Arc<Mutex<HashMap<Identifier, VecDeque<oneshot::Sender<()>>>>>,
    resource_identifier: Identifier,
) -> oneshot::Receiver<()> {
    // Set up a oneshot channel to be notified when it becomes available
    let (tx, rx) = oneshot::channel();
    let mut queues = queues.lock().await;
    if let Some(queue) = queues.get_mut(&resource_identifier) {
        queue.push_back(tx);
    } else {
        let mut queue = VecDeque::new();
        queue.push_back(tx);
        queues.insert(resource_identifier.clone(), queue);
    }
    rx
}
