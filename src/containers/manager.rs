use std::collections::{HashMap, VecDeque};
use tokio::sync::{mpsc, oneshot};

use crate::identifiers::Identifier;

use super::{Container, ContainerError};

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
        if !self.containers.contains_key(&resource_identifier) {
            let container = Container::new();
            self.containers.insert(resource_identifier, container);
            true
        } else {
            false
        }
    }

    /// Attempt to acquire a read lock on the [`Container`] registered with the provided key.
    ///
    /// If the [`Container`] is available, it is reserved for reading and `Ok` is returned.
    /// If the [`Container`] is already reserved for writing, an error is returned.
    ///
    /// # Errors
    /// If there is no [`Container`] registered with the provided key an error is returned.
    pub fn try_read(&mut self, resource_identifier: Identifier) -> Result<bool, ContainerError> {
        if let Some(container) = self.containers.get_mut(&resource_identifier) {
            if container.reserve_read() {
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            Err(ContainerError::NotRegistered(resource_identifier.clone()))
        }
    }

    /// Attempt to acquire a write lock on the [`Container`] registered with the provided key.
    ///
    /// If the [`Container`] is available, it is reserved for writing and `Ok` is returned.
    /// If the [`Container`] is already reserved, an error is returned.
    ///
    /// # Errors
    /// If there is no [`Container`] registered with the provided key an error is returned.
    pub fn try_write(&mut self, resource_identifier: Identifier) -> Result<bool, ContainerError> {
        if let Some(container) = self.containers.get_mut(&resource_identifier) {
            if container.reserve_write() {
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            Err(ContainerError::NotRegistered(resource_identifier.clone()))
        }
    }

    /// Release the reservation on the [`Container`] registered with the provided key.
    ///
    /// If the [`Container`] is reserved, it is set as available and `Ok(())` is returned.
    ///
    /// # Errors
    /// If the [`Container`] is already available or not registered, an error is returned.
    pub fn try_release(&mut self, resource_identifier: Identifier) -> Result<(), ContainerError> {
        if let Some(container) = self.containers.get_mut(&resource_identifier) {
            if container.is_write() {
                if container.release_write() {
                    Ok(())
                } else {
                    Err(ContainerError::NotReserved(resource_identifier.clone()))
                }
            } else if container.is_read() {
                if container.release_read() {
                    Ok(())
                } else {
                    Err(ContainerError::NotReserved(resource_identifier.clone()))
                }
            } else {
                Err(ContainerError::NotReserved(resource_identifier.clone()))
            }
        } else {
            Err(ContainerError::NotRegistered(resource_identifier.clone()))
        }
    }
}

pub enum ContainerAction {
    Register(Identifier, oneshot::Sender<ContainerResult>),
    ReserveRead(Identifier, oneshot::Sender<ContainerResult>),
    ReserveWrite(Identifier, oneshot::Sender<ContainerResult>),
    Release(Identifier, oneshot::Sender<ContainerResult>),
}

#[derive(Debug)]
pub enum ContainerResult {
    Done,
    Wait(oneshot::Receiver<()>),
    Error(ContainerError),
}

pub async fn containers_manager(mut receiver: mpsc::Receiver<ContainerAction>) {
    let mut queues = HashMap::new();
    let mut manager = ContainersManager::default();
    while let Some(message) = receiver.recv().await {
        match message {
            ContainerAction::Register(identifier, responder) => {
                manager.register(identifier);
                responder.send(ContainerResult::Done).unwrap();
            }
            ContainerAction::ReserveRead(identifier, responder) => {
                match manager.try_read(identifier.clone()) {
                    Ok(true) => {
                        responder.send(ContainerResult::Done).unwrap();
                    }
                    Ok(false) => {
                        let callback = wait_container_release(&mut queues, identifier).await;
                        responder.send(ContainerResult::Wait(callback)).unwrap();
                    }
                    Err(e) => {
                        responder.send(ContainerResult::Error(e)).unwrap();
                    }
                };
            }
            ContainerAction::ReserveWrite(identifier, responder) => {
                match manager.try_write(identifier.clone()) {
                    Ok(true) => {
                        responder.send(ContainerResult::Done).unwrap();
                    }
                    Ok(false) => {
                        let callback = wait_container_release(&mut queues, identifier).await;
                        responder.send(ContainerResult::Wait(callback)).unwrap();
                    }
                    Err(e) => {
                        responder.send(ContainerResult::Error(e)).unwrap();
                    }
                };
            }
            ContainerAction::Release(identifier, responder) => {
                if let Some(channel) = queues
                    .get_mut(&identifier)
                    .and_then(|queue| queue.pop_front())
                {
                    channel.send(()).unwrap();
                    responder.send(ContainerResult::Done).unwrap();
                } else {
                    match manager.try_release(identifier.clone()) {
                        Ok(()) => responder.send(ContainerResult::Done).unwrap(),
                        Err(e) => responder.send(ContainerResult::Error(e)).unwrap(),
                    }
                }
            }
        }
    }
}

async fn wait_container_release(
    queues: &mut HashMap<Identifier, VecDeque<oneshot::Sender<()>>>,
    resource_identifier: Identifier,
) -> oneshot::Receiver<()> {
    // Set up a oneshot channel to be notified when it becomes available
    let (tx, rx) = oneshot::channel();
    if let Some(queue) = queues.get_mut(&resource_identifier) {
        queue.push_back(tx);
    } else {
        let mut queue = VecDeque::new();
        queue.push_back(tx);
        queues.insert(resource_identifier.clone(), queue);
    }
    rx
}
