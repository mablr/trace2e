use std::{fmt::Error, sync::Arc};

use tokio::sync::{mpsc, oneshot, Mutex};

use crate::{
    containers::{ContainerAction, ContainerResult},
    identifiers::Identifier,
};

#[derive(Debug, Clone)]
pub struct Flow {
    pub id: u64,
    pub source: Identifier,
    pub destination: Identifier,
}

#[derive(Debug)]
pub struct Provenance {
    containers_manager: mpsc::Sender<ContainerAction>,
    grant_counter: Arc<Mutex<u64>>,
}

impl Provenance {
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
    ) -> Result<Flow, Error> {
        let (tx, rx) = oneshot::channel();
        let _ = self
            .containers_manager
            .send(ContainerAction::ReserveRead(source.clone(), tx))
            .await;
        match rx.await.unwrap() {
            ContainerResult::Wait(callback) => {
                #[cfg(feature = "verbose")]
                println!("⏸️  read wait {}", source.clone());
                callback.await.unwrap();
                #[cfg(feature = "verbose")]
                println!("⏯️  read got after wait {}", source.clone())
            }
            ContainerResult::Done => {
                #[cfg(feature = "verbose")]
                println!("⏩ read got {}", source.clone());
            }
            ContainerResult::Error(_) => todo!(),
        }
        let (tx, rx) = oneshot::channel();
        let _ = self
            .containers_manager
            .send(ContainerAction::ReserveWrite(destination.clone(), tx))
            .await;
        match rx.await.unwrap() {
            ContainerResult::Wait(callback) => {
                #[cfg(feature = "verbose")]
                println!("⏸️  write wait {}", destination.clone());
                callback.await.unwrap();
                #[cfg(feature = "verbose")]
                println!("⏯️  write got after wait {}", destination.clone());
            }
            ContainerResult::Done => {
                #[cfg(feature = "verbose")]
                println!("⏩ write got {}", destination.clone());
            }
            ContainerResult::Error(_) => todo!(),
        }

        Ok(Flow {
            id: self.get_grant_id().await,
            source,
            destination,
        })
    }

    pub async fn record_flow(&self, flow: Flow) -> Result<(), Error> {
        let (tx, rx) = oneshot::channel();
        self.containers_manager
            .send(ContainerAction::Release(flow.source, tx))
            .await
            .unwrap();
        rx.await.unwrap();
        let (tx, rx) = oneshot::channel();
        self.containers_manager
            .send(ContainerAction::Release(flow.destination, tx))
            .await
            .unwrap();
        rx.await.unwrap();
        Ok(())
    }
}
