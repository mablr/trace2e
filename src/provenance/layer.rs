use std::sync::Arc;

use tokio::sync::{mpsc, oneshot, Mutex};

use crate::{
    containers::{ContainerAction, ContainerReservationResult, ContainerResult},
    identifiers::Identifier,
};

use super::{Flow, ProvenanceError};

#[derive(Debug)]
pub struct ProvenanceLayer {
    containers_manager: mpsc::Sender<ContainerAction>,
    grant_counter: Arc<Mutex<u64>>,
}

impl ProvenanceLayer {
    pub fn new(containers_manager: mpsc::Sender<ContainerAction>) -> Self {
        Self {
            containers_manager,
            grant_counter: Arc::new(Mutex::new(0)),
        }
    }

    async fn get_grant_id(&self) -> u64 {
        let mut grant_id = self.grant_counter.lock().await;
        *grant_id += 1;
        *grant_id
    }

    pub async fn declare_flow(
        &self,
        source: Identifier,
        destination: Identifier,
    ) -> Result<Flow, ProvenanceError> {
        let (tx, rx) = oneshot::channel();
        let _ = self
            .containers_manager
            .send(ContainerAction::ReserveRead(source.clone(), tx))
            .await;
        match rx.await.unwrap() {
            ContainerReservationResult::Wait(callback) => {
                #[cfg(feature = "verbose")]
                println!("⏸️  read wait {}", source.clone());
                callback.await.unwrap();
                #[cfg(feature = "verbose")]
                println!("⏯️  read got after wait {}", source.clone())
            }
            ContainerReservationResult::Error(e) => {
                return Err(ProvenanceError::ContainerFailure(e))
            }
        };
        let (tx, rx) = oneshot::channel();
        let _ = self
            .containers_manager
            .send(ContainerAction::ReserveWrite(destination.clone(), tx))
            .await;
        match rx.await.unwrap() {
            ContainerReservationResult::Wait(callback) => {
                #[cfg(feature = "verbose")]
                println!("⏸️  write wait {}", source.clone());
                callback.await.unwrap();
                #[cfg(feature = "verbose")]
                println!("⏯️  write got after wait {}", source.clone())
            }
            ContainerReservationResult::Error(e) => {
                return Err(ProvenanceError::ContainerFailure(e))
            }
        };

        Ok(Flow {
            id: self.get_grant_id().await,
            source,
            destination,
        })
    }

    pub async fn record_flow(&self, flow: Flow) -> Result<(), ProvenanceError> {
        let (tx, rx) = oneshot::channel();

        self.containers_manager
            .send(ContainerAction::Release(flow.source.clone(), tx))
            .await
            .unwrap();
        match rx.await.unwrap() {
            ContainerResult::Done => {
                #[cfg(feature = "verbose")]
                println!("⏯️  release {}", flow.source);
            }
            ContainerResult::Error(e) => return Err(ProvenanceError::ContainerFailure(e)),
        }
        let (tx, rx) = oneshot::channel();
        self.containers_manager
            .send(ContainerAction::Release(flow.destination.clone(), tx))
            .await
            .unwrap();
        match rx.await.unwrap() {
            ContainerResult::Done => {
                #[cfg(feature = "verbose")]
                println!("⏯️  release {}", flow.destination);
            }
            ContainerResult::Error(e) => return Err(ProvenanceError::ContainerFailure(e)),
        }
        Ok(())
    }
}
