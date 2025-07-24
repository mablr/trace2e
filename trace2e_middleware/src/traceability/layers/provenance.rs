use tokio::sync::Mutex;
use tower::Service;

use crate::traceability::{
    api::{ProvenanceRequest, ProvenanceResponse},
    error::TraceabilityError,
    naming::{Fd, Identifier, Resource},
};
use std::{
    collections::{HashMap, HashSet},
    pin::Pin,
    sync::Arc,
    task::Poll,
};

fn init_provenance(id: Identifier) -> HashSet<Identifier> {
    match id.resource {
        Resource::Process(_) | Resource::Fd(Fd::File(_)) => HashSet::from([id]),
        _ => HashSet::new(),
    }
}

#[derive(Debug, Default, Clone)]
pub struct ProvenanceService {
    derived_from_map: Arc<Mutex<HashMap<Identifier, HashSet<Identifier>>>>,
}

impl ProvenanceService {
    /// Get the provenance of an identifier
    async fn get_prov(&self, id: Identifier) -> HashSet<Identifier> {
        self.derived_from_map
            .lock()
            .await
            .get(&id)
            .cloned()
            .unwrap_or(init_provenance(id))
    }

    /// Update the provenance of the destination with the source
    ///
    /// Note that this function does not guarantee sequential consistency,
    /// this is the role of the sequencer.
    async fn update(&mut self, source: Identifier, destination: Identifier) -> ProvenanceResponse {
        let source_prov = self.get_prov(source.clone()).await;
        let destination_prov = self.get_prov(destination.clone()).await;
        if destination_prov.contains(&source) {
            ProvenanceResponse::ProvenanceNotUpdated
        } else {
            self.derived_from_map.lock().await.insert(
                destination,
                destination_prov.union(&source_prov).cloned().collect(),
            );
            ProvenanceResponse::ProvenanceUpdated
        }
    }
}

impl Service<ProvenanceRequest> for ProvenanceService {
    type Response = ProvenanceResponse;
    type Error = TraceabilityError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: ProvenanceRequest) -> Self::Future {
        let mut this = self.clone();
        Box::pin(async move {
            match req.clone() {
                ProvenanceRequest::GetProvenance { id } => Ok(ProvenanceResponse::Provenance {
                    derived_from: this.get_prov(id.clone()).await,
                }),
                ProvenanceRequest::UpdateProvenance {
                    source,
                    destination,
                } => Ok(this.update(source, destination).await),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use tower::ServiceBuilder;

    use super::*;

    #[tokio::test]
    async fn unit_provenance_update_simple() {
        let mut provenance = ProvenanceService::default();
        let node_id = "test".to_string();
        let process = Identifier::new(node_id.clone(), Resource::new_process(0));
        let file = Identifier::new(node_id.clone(), Resource::new_file("/tmp/test".to_string()));

        provenance.update(file.clone(), process.clone()).await;

        // Check that the process is now derived from the file
        assert_eq!(
            provenance.get_prov(process.clone()).await,
            HashSet::from([file.clone(), process.clone()])
        );
    }

    #[tokio::test]
    async fn unit_provenance_update_circular() {
        let mut provenance = ProvenanceService::default();
        let node_id = "test".to_string();
        let process = Identifier::new(node_id.clone(), Resource::new_process(0));
        let file = Identifier::new(node_id.clone(), Resource::new_file("/tmp/test".to_string()));

        provenance.update(process.clone(), file.clone()).await;
        provenance.update(file.clone(), process.clone()).await;

        // Check the proper handling of circular dependencies
        assert_eq!(
            provenance.get_prov(file.clone()).await,
            provenance.get_prov(process.clone()).await
        );
    }

    #[tokio::test]
    async fn unit_provenance_service_flow_simple() {
        let mut provenance = ServiceBuilder::new().service(ProvenanceService::default());
        let node_id = "test".to_string();
        let process = Identifier::new(node_id.clone(), Resource::new_process(0));
        let file = Identifier::new(node_id.clone(), Resource::new_file("/tmp/test".to_string()));

        assert_eq!(
            provenance
                .call(ProvenanceRequest::GetProvenance {
                    id: process.clone()
                })
                .await
                .unwrap(),
            ProvenanceResponse::Provenance {
                derived_from: HashSet::from([process.clone()]),
            }
        );

        assert_eq!(
            provenance
                .call(ProvenanceRequest::UpdateProvenance {
                    source: file.clone(),
                    destination: process.clone(),
                })
                .await
                .unwrap(),
            ProvenanceResponse::ProvenanceUpdated
        );

        assert_eq!(
            provenance
                .call(ProvenanceRequest::UpdateProvenance {
                    source: file.clone(),
                    destination: process.clone(),
                })
                .await
                .unwrap(),
            ProvenanceResponse::ProvenanceNotUpdated
        );

        assert_eq!(
            provenance
                .call(ProvenanceRequest::GetProvenance {
                    id: process.clone()
                })
                .await
                .unwrap(),
            ProvenanceResponse::Provenance {
                derived_from: HashSet::from([file.clone(), process.clone()]),
            }
        );
    }
}
