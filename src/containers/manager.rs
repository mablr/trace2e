use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};
use tokio::{
    sync::{mpsc, oneshot, Mutex, RwLock},
    time::{timeout, Duration},
};

use crate::identifiers::Identifier;

use super::ContainerError;

pub enum ContainerAction {
    Register(Identifier, oneshot::Sender<ContainerResult>),
    ReserveRead(Identifier, oneshot::Sender<ContainerReservationResult>),
    ReserveWrite(Identifier, oneshot::Sender<ContainerReservationResult>),
    Release(Identifier, oneshot::Sender<ContainerResult>),
}

#[derive(Debug)]
pub enum ContainerReservationResult {
    Wait(oneshot::Receiver<()>),
    Error(ContainerError),
}

impl std::cmp::PartialEq for ContainerReservationResult {
    fn eq(&self, other: &Self) -> bool {
        match self {
            ContainerReservationResult::Wait(_) => {
                matches!(other, ContainerReservationResult::Wait(_))
            }
            ContainerReservationResult::Error(e_self) => match other {
                ContainerReservationResult::Error(e_other) => e_self == e_other,
                _ => false,
            },
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ContainerResult {
    Done,
    Error(ContainerError),
}

pub async fn containers_manager(mut receiver: mpsc::Receiver<ContainerAction>) {
    let mut containers = HashMap::new();
    let containers_read_release_handles = Arc::new(Mutex::new(HashMap::new()));
    let containers_write_release_handles = Arc::new(Mutex::new(HashMap::new()));

    while let Some(message) = receiver.recv().await {
        match message {
            ContainerAction::Register(identifier, responder) => {
                if !containers.contains_key(&identifier) {
                    containers.insert(identifier, Arc::new(RwLock::new(())));
                }
                responder.send(ContainerResult::Done).unwrap();
            }
            ContainerAction::ReserveRead(identifier, responder) => {
                if let Some(container) = containers.get(&identifier).cloned() {
                    let containers_read_release_handles =
                        Arc::clone(&containers_read_release_handles);
                    tokio::spawn(async move {
                        let (reservation_result, reservation) = oneshot::channel();
                        responder
                            .send(ContainerReservationResult::Wait(reservation))
                            .unwrap();
                        let guard = container.read().await;
                        let (release_callback, release) = oneshot::channel();
                        reservation_result.send(()).unwrap();
                        containers_read_release_handles
                            .lock()
                            .await
                            .entry(identifier.clone())
                            .or_insert_with(VecDeque::new)
                            .push_back(release_callback);

                        if timeout(Duration::from_millis(50), release).await.is_err() {
                            todo!() // kill the blocking process
                        }
                        drop(guard);
                    });
                } else {
                    responder
                        .send(ContainerReservationResult::Error(
                            ContainerError::NotRegistered(identifier),
                        ))
                        .unwrap();
                }
            }
            ContainerAction::ReserveWrite(identifier, responder) => {
                if let Some(container) = containers.get(&identifier).cloned() {
                    let containers_write_release_handles =
                        Arc::clone(&containers_write_release_handles);
                    tokio::spawn(async move {
                        let (reservation_result, reservation) = oneshot::channel();
                        responder
                            .send(ContainerReservationResult::Wait(reservation))
                            .unwrap();
                        let guard = container.write().await;
                        let (release_callback, release) = oneshot::channel();
                        reservation_result.send(()).unwrap();
                        containers_write_release_handles
                            .lock()
                            .await
                            .insert(identifier.clone(), release_callback);
                        if timeout(Duration::from_millis(50), release).await.is_err() {
                            todo!() // kill the blocking process
                        }
                        drop(guard);
                    });
                } else {
                    responder
                        .send(ContainerReservationResult::Error(
                            ContainerError::NotRegistered(identifier),
                        ))
                        .unwrap();
                }
            }
            ContainerAction::Release(identifier, responder) => {
                if containers.contains_key(&identifier) {
                    if let Some(handle) = containers_read_release_handles
                        .lock()
                        .await
                        .get_mut(&identifier)
                        .and_then(|queue| queue.pop_front())
                    {
                        handle.send(()).unwrap();
                        responder.send(ContainerResult::Done).unwrap();
                    } else if let Some(handle) = containers_write_release_handles
                        .lock()
                        .await
                        .remove(&identifier)
                    {
                        handle.send(()).unwrap();
                        responder.send(ContainerResult::Done).unwrap();
                    } else {
                        responder
                            .send(ContainerResult::Error(ContainerError::NotReserved(
                                identifier,
                            )))
                            .unwrap();
                    }
                } else {
                    responder
                        .send(ContainerResult::Error(ContainerError::NotRegistered(
                            identifier,
                        )))
                        .unwrap();
                }
            }
        }
    }
}
