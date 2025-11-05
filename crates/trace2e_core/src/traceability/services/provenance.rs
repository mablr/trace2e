//! Provenance service for tracking resource references across nodes.
//!
//! Provides async helpers to get/update provenance and a tower::Service implementation.
use std::{collections::HashSet, pin::Pin, sync::Arc, task::Poll};

use dashmap::DashMap;
use tower::Service;

#[cfg(feature = "trace2e_tracing")]
use crate::traceability::infrastructure::naming::DisplayableResource;
#[cfg(feature = "trace2e_tracing")]
use tracing::info;

use crate::traceability::{
    api::types::{ProvenanceRequest, ProvenanceResponse},
    error::TraceabilityError,
    infrastructure::naming::{LocalizedResource, NodeId, Resource},
};

type ProvenanceMap = DashMap<Resource, HashSet<LocalizedResource>>;

/// Provenance service for tracking resources provenance
#[derive(Debug, Default, Clone)]
pub struct ProvenanceService {
    node_id: String,
    provenance: Arc<ProvenanceMap>,
}

impl ProvenanceService {
    pub fn new(node_id: String) -> Self {
        Self { node_id, provenance: Arc::new(DashMap::new()) }
    }

    fn init_provenance(&self, resource: &Resource) -> HashSet<LocalizedResource> {
        if !resource.is_stream() {
            HashSet::from([LocalizedResource::new(self.node_id.clone(), resource.to_owned())])
        } else {
            HashSet::new()
        }
    }

    /// Get the provenance of a resource
    ///
    /// This function returns a map of node IDs to the provenance of the resource for that node.
    /// If the resource is found, it initializes the provenance for the resource.
    fn get_prov(&self, resource: &Resource) -> HashSet<LocalizedResource> {
        if let Some(prov) = self.provenance.get(resource) {
            prov.to_owned()
        } else {
            self.init_provenance(resource)
        }
    }

    /// Update the provenance of the destination with the source
    ///
    /// Note that this function does not guarantee sequential consistency,
    /// this is the role of the sequencer.
    fn update(&mut self, source: &Resource, destination: &Resource) -> ProvenanceResponse {
        // Update the provenance of the destination with the source provenance
        self.update_raw(self.get_prov(source), destination)
    }

    /// Update the provenance of the destination with the raw source provenance
    ///
    /// Note that this function does not guarantee sequential consistency,
    /// this is the role of the sequencer.
    fn update_raw(
        &mut self,
        source_prov: HashSet<LocalizedResource>,
        destination: &Resource,
    ) -> ProvenanceResponse {
        let mut destination_prov = self.get_prov(destination);
        if source_prov.is_subset(&destination_prov) {
            #[cfg(feature = "trace2e_tracing")]
            info!(
                "[provenance-raw] Provenance not updated: source_prov is subset of destination_prov"
            );
            ProvenanceResponse::ProvenanceNotUpdated
        } else {
            destination_prov.extend(source_prov);
            #[cfg(feature = "trace2e_tracing")]
            info!(
                "[provenance-raw] Provenance updated: destination_prov: {}",
                DisplayableResource::from(&destination_prov)
            );
            self.provenance.insert(destination.to_owned(), destination_prov);
            ProvenanceResponse::ProvenanceUpdated
        }
    }
}

impl NodeId for ProvenanceService {
    fn node_id(&self) -> String {
        self.node_id.to_owned()
    }
}

impl Service<ProvenanceRequest> for ProvenanceService {
    type Response = ProvenanceResponse;
    type Error = TraceabilityError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: ProvenanceRequest) -> Self::Future {
        let mut this = self.clone();
        Box::pin(async move {
            match request {
                ProvenanceRequest::GetReferences(resource) => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!("[provenance-{}] GetReferences: {}", this.node_id, resource);
                    Ok(ProvenanceResponse::Provenance(this.get_prov(&resource)))
                }
                ProvenanceRequest::UpdateProvenance { source, destination } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[provenance-{}] UpdateProvenance: source: {}, destination: {}",
                        this.node_id, source, destination
                    );
                    Ok(this.update(&source, &destination))
                }
                ProvenanceRequest::UpdateProvenanceRaw { source_prov, destination } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[provenance-{}] UpdateProvenanceRaw: source_prov: {}, destination: {}",
                        this.node_id,
                        DisplayableResource::from(&source_prov),
                        destination
                    );
                    Ok(this.update_raw(source_prov, &destination))
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_provenance_update_simple() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut provenance = ProvenanceService::default();
        let process = LocalizedResource::new(provenance.node_id(), Resource::new_process_mock(0));
        let file = LocalizedResource::new(
            provenance.node_id(),
            Resource::new_file("/tmp/test".to_string()),
        );

        assert_eq!(
            provenance.update(file.resource(), process.resource()),
            ProvenanceResponse::ProvenanceUpdated
        );
        // Check that the process is now derived from the file
        assert_eq!(provenance.get_prov(process.resource()), HashSet::from([file, process]));
    }

    #[test]
    fn unit_provenance_update_circular() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut provenance = ProvenanceService::default();
        let process = LocalizedResource::new(provenance.node_id(), Resource::new_process_mock(0));
        let file = LocalizedResource::new(
            provenance.node_id(),
            Resource::new_file("/tmp/test".to_string()),
        );

        assert_eq!(
            provenance.update(process.resource(), file.resource()),
            ProvenanceResponse::ProvenanceUpdated
        );
        assert_eq!(
            provenance.update(file.resource(), process.resource()),
            ProvenanceResponse::ProvenanceUpdated
        );

        // Check the proper handling of circular dependencies
        assert_eq!(provenance.get_prov(file.resource()), provenance.get_prov(process.resource()));
    }

    #[tokio::test]
    async fn unit_provenance_service_flow_simple() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut provenance = ProvenanceService::default();
        let process = LocalizedResource::new(provenance.node_id(), Resource::new_process_mock(0));
        let file = LocalizedResource::new(
            provenance.node_id(),
            Resource::new_file("/tmp/test".to_string()),
        );

        assert_eq!(
            provenance
                .call(ProvenanceRequest::GetReferences(process.resource().clone()))
                .await
                .unwrap(),
            ProvenanceResponse::Provenance(HashSet::from([process.clone()]))
        );

        assert_eq!(
            provenance
                .call(ProvenanceRequest::UpdateProvenance {
                    source: file.resource().clone(),
                    destination: process.resource().clone(),
                })
                .await
                .unwrap(),
            ProvenanceResponse::ProvenanceUpdated
        );

        assert_eq!(
            provenance
                .call(ProvenanceRequest::UpdateProvenance {
                    source: file.resource().clone(),
                    destination: process.resource().clone(),
                })
                .await
                .unwrap(),
            ProvenanceResponse::ProvenanceNotUpdated
        );

        assert_eq!(
            provenance
                .call(ProvenanceRequest::GetReferences(process.resource().clone()))
                .await
                .unwrap(),
            ProvenanceResponse::Provenance(HashSet::from([file, process,]))
        );
    }
}
