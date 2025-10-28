//! Provenance service for tracking resource references across nodes.
//!
//! Provides async helpers to get/update provenance and a tower::Service implementation.
use std::{collections::HashSet, pin::Pin, sync::Arc, task::Poll};

use dashmap::DashMap;
use tower::Service;
#[cfg(feature = "trace2e_tracing")]
use tracing::{debug, info};

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
    /// If the resource is not found, it returns an empty map.
    async fn get_prov(&self, resource: &Resource) -> HashSet<LocalizedResource> {
        if let Some(prov) = self.provenance.get(resource) {
            prov.to_owned()
        } else {
            self.init_provenance(resource)
        }
    }

    /// Set the provenance of a resource
    ///
    /// Returns `true` if the provenance changed, `false` if it was the same.
    async fn set_prov(
        &mut self,
        resource: Resource,
        prov: HashSet<LocalizedResource>,
    ) -> ProvenanceResponse {
        // Check if the provenance is different from the current one
        if let Some(current_prov) = self.provenance.get(&resource) {
            if current_prov.value() == &prov {
                return ProvenanceResponse::ProvenanceNotUpdated; // No change
            }
        }
        self.provenance.insert(resource, prov);
        #[cfg(feature = "trace2e_tracing")]
        debug!("[provenance-raw] Updated {:?} provenance: {:?}", resource, prov);
        ProvenanceResponse::ProvenanceUpdated // Provenance was updated
    }

    /// Update the provenance of the destination with the source
    ///
    /// Note that this function does not guarantee sequential consistency,
    /// this is the role of the sequencer.
    async fn update(&mut self, source: &Resource, destination: &Resource) -> ProvenanceResponse {
        // Record the node IDs to which local resources propagate
        if !destination.is_stream() {
            // Update the provenance of the destination with the source provenance
            self.update_raw(self.get_prov(source).await, destination).await
        } else {
            ProvenanceResponse::ProvenanceNotUpdated
        }
    }

    /// Update the provenance of the destination with the raw source provenance
    ///
    /// Note that this function does not guarantee sequential consistency,
    /// this is the role of the sequencer.
    async fn update_raw(
        &mut self,
        source_prov: HashSet<LocalizedResource>,
        destination: &Resource,
    ) -> ProvenanceResponse {
        let mut destination_prov = self.get_prov(destination).await;
        #[cfg(feature = "trace2e_tracing")]
        debug!("[provenance-raw] Previous {:?} provenance: {:?}", destination, destination_prov);
        destination_prov.extend(source_prov);
        self.set_prov(destination.to_owned(), destination_prov).await
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
                    info!("[provenance-{}] GetReferences: {:?}", this.node_id, resource);
                    Ok(ProvenanceResponse::Provenance(this.get_prov(&resource).await))
                }
                ProvenanceRequest::UpdateProvenance { source, destination } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[provenance-{}] UpdateProvenance: source: {:?}, destination: {:?}",
                        this.node_id, source, destination
                    );
                    Ok(this.update(&source, &destination).await)
                }
                ProvenanceRequest::UpdateProvenanceRaw { source_prov, destination } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[provenance-{}] UpdateProvenanceRaw: source_prov: {:?}, destination: {:?}",
                        this.node_id, source_prov, destination
                    );
                    Ok(this.update_raw(source_prov, &destination).await)
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unit_provenance_update_simple() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut provenance = ProvenanceService::default();
        let process = Resource::new_process_mock(0);
        let file = Resource::new_file("/tmp/test".to_string());

        assert_eq!(provenance.update(&file, &process).await, ProvenanceResponse::ProvenanceUpdated);
        // Check that the process is now derived from the file
        assert_eq!(
            provenance.get_prov(&process).await,
            HashSet::from([
                LocalizedResource::new(provenance.node_id(), file),
                LocalizedResource::new(provenance.node_id(), process)
            ])
        );
    }

    #[tokio::test]
    async fn unit_provenance_update_circular() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut provenance = ProvenanceService::default();
        let process = Resource::new_process_mock(0);
        let file = Resource::new_file("/tmp/test".to_string());

        assert_eq!(provenance.update(&process, &file).await, ProvenanceResponse::ProvenanceUpdated);
        assert_eq!(provenance.update(&file, &process).await, ProvenanceResponse::ProvenanceUpdated);

        // Check the proper handling of circular dependencies
        assert_eq!(provenance.get_prov(&file).await, provenance.get_prov(&process).await);
    }

    #[tokio::test]
    async fn unit_provenance_update_multiple_nodes() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut provenance = ProvenanceService::default();
        let process0 = Resource::new_process_mock(0);
        let process1 = Resource::new_process_mock(1);
        let file0 = Resource::new_file("/tmp/test0".to_string());

        assert_eq!(
            provenance
                .update_raw(
                    HashSet::from([
                        LocalizedResource::new("10.0.0.1".to_string(), process0.clone()),
                        LocalizedResource::new("10.0.0.2".to_string(), process0.clone()),
                    ]),
                    &process0,
                )
                .await,
            ProvenanceResponse::ProvenanceUpdated
        );
        assert_eq!(
            provenance
                .update_raw(
                    HashSet::from([
                        LocalizedResource::new("10.0.0.1".to_string(), process1.clone()),
                        LocalizedResource::new("10.0.0.2".to_string(), file0.clone()),
                        LocalizedResource::new("10.0.0.2".to_string(), process1.clone()),
                    ]),
                    &process0,
                )
                .await,
            ProvenanceResponse::ProvenanceUpdated
        );

        assert_eq!(
            provenance.get_prov(&process0).await,
            HashSet::from([
                LocalizedResource::new(provenance.node_id(), process0.clone()),
                LocalizedResource::new("10.0.0.1".to_string(), process0.clone()),
                LocalizedResource::new("10.0.0.1".to_string(), process1.clone()),
                LocalizedResource::new("10.0.0.2".to_string(), file0),
                LocalizedResource::new("10.0.0.2".to_string(), process0),
                LocalizedResource::new("10.0.0.2".to_string(), process1),
            ])
        );
    }

    #[tokio::test]
    async fn unit_provenance_service_flow_simple() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut provenance = ProvenanceService::default();
        let process = Resource::new_process_mock(0);
        let file = Resource::new_file("/tmp/test".to_string());

        assert_eq!(
            provenance.call(ProvenanceRequest::GetReferences(process.clone())).await.unwrap(),
            ProvenanceResponse::Provenance(HashSet::from([LocalizedResource::new(
                provenance.node_id(),
                process.clone()
            ),]))
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
            provenance.call(ProvenanceRequest::GetReferences(process.clone())).await.unwrap(),
            ProvenanceResponse::Provenance(HashSet::from([
                LocalizedResource::new(provenance.node_id(), file),
                LocalizedResource::new(provenance.node_id(), process),
            ]))
        );
    }
}
