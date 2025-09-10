use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
    task::Poll,
};

use dashmap::DashMap;
use tower::Service;
#[cfg(feature = "trace2e_tracing")]
use tracing::{debug, info};

use crate::traceability::{
    api::{ProvenanceRequest, ProvenanceResponse},
    error::TraceabilityError,
    mode::Mode,
    naming::{Fd, NodeId, Resource},
};

type ProvenanceMap = DashMap<Resource, HashMap<String, HashSet<Resource>>>;
type PropagationMap = DashMap<Resource, HashSet<String>>;

/// Provenance service for tracking resources provenance
#[derive(Debug, Default, Clone)]
pub struct ProvenanceService {
    node_id: String,
    provenance: Arc<ProvenanceMap>,
    propagation: Arc<PropagationMap>,
    mode: Mode,
}

impl ProvenanceService {
    pub fn new(node_id: String) -> Self {
        Self {
            node_id,
            provenance: Arc::new(DashMap::new()),
            propagation: Arc::new(DashMap::new()),
            mode: Default::default(),
        }
    }

    fn init_provenance(&self, resource: &Resource) -> HashMap<String, HashSet<Resource>> {
        if resource.is_stream().is_none() {
            HashMap::from([(self.node_id.clone(), HashSet::from([resource.to_owned()]))])
        } else {
            HashMap::new()
        }
    }

    pub fn with_mode(self, mode: Mode) -> Self {
        Self { mode, ..self }
    }

    /// Get the provenance of a resource
    ///
    /// This function returns a map of node IDs to the provenance of the resource for that node.
    /// If the resource is not found, it returns an empty map.
    async fn get_prov(&self, resource: &Resource) -> HashMap<String, HashSet<Resource>> {
        if let Some(prov) = self.provenance.get(resource) {
            prov.to_owned()
        } else {
            self.init_provenance(resource)
        }
    }

    /// Set the provenance of a resource
    async fn set_prov(&mut self, resource: Resource, prov: HashMap<String, HashSet<Resource>>) {
        self.provenance.insert(resource, prov);
    }

    /// Update the provenance of the destination with the source
    ///
    /// Note that this function does not guarantee sequential consistency,
    /// this is the role of the sequencer.
    async fn update(
        &mut self,
        source: &Resource,
        destination: &Resource,
    ) -> Result<ProvenanceResponse, TraceabilityError> {
        let source_prov = self.get_prov(source).await;
        // Record the node IDs to which local resources propagate
        if let Resource::Fd(Fd::Stream(stream)) = destination
            && self.mode == Mode::Push
        {
            match stream.peer_socket.parse::<SocketAddr>() {
                Ok(addr) => {
                    if let Some(local_sources) = source_prov.get(&self.node_id) {
                        for local_source in local_sources {
                            if let Some(mut local_sources) = self.propagation.get_mut(local_source)
                            {
                                local_sources.insert(addr.ip().to_string());
                            } else {
                                self.propagation.insert(
                                    local_source.to_owned(),
                                    HashSet::from([addr.ip().to_string()]),
                                );
                            }
                        }
                    }
                }
                Err(_) => return Err(TraceabilityError::TransportFailedToEvaluateRemote),
            }
            Ok(ProvenanceResponse::ProvenanceUpdated)
        } else if destination.is_stream().is_none() {
            // Update the provenance of the destination with the source provenance
            self.update_raw(source_prov, destination).await
        } else {
            Ok(ProvenanceResponse::ProvenanceNotUpdated)
        }
    }

    /// Update the provenance of the destination with the raw source provenance
    ///
    /// Note that this function does not guarantee sequential consistency,
    /// this is the role of the sequencer.
    async fn update_raw(
        &mut self,
        source_prov: HashMap<String, HashSet<Resource>>,
        destination: &Resource,
    ) -> Result<ProvenanceResponse, TraceabilityError> {
        let mut updated = false;
        let mut destination_prov = self.get_prov(destination).await;
        #[cfg(feature = "trace2e_tracing")]
        debug!("[provenance-raw] Previous {:?} provenance: {:?}", destination, destination_prov);
        for (node_id, node_source_prov) in source_prov {
            if let Some(node_destination_prov) = destination_prov.get_mut(&node_id) {
                if !node_destination_prov.is_superset(&node_source_prov) {
                    node_destination_prov.extend(node_source_prov);
                    updated = true;
                }
            } else {
                destination_prov.insert(node_id, node_source_prov);
                updated = true;
            }
        }
        if updated {
            #[cfg(feature = "trace2e_tracing")]
            debug!("[provenance-raw] Updated {:?} provenance: {:?}", destination, destination_prov);
            self.set_prov(destination.to_owned(), destination_prov).await;
            Ok(ProvenanceResponse::ProvenanceUpdated)
        } else {
            Ok(ProvenanceResponse::ProvenanceNotUpdated)
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
                    info!("[provenance-{}] GetReferences: {:?}", this.node_id, resource);
                    Ok(ProvenanceResponse::Provenance(this.get_prov(&resource).await))
                }
                ProvenanceRequest::UpdateProvenance { source, destination } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[provenance-{}] UpdateProvenance: source: {:?}, destination: {:?}",
                        this.node_id, source, destination
                    );
                    this.update(&source, &destination).await
                }
                ProvenanceRequest::UpdateProvenanceRaw { source_prov, destination } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[provenance-{}] UpdateProvenanceRaw: source_prov: {:?}, destination: {:?}",
                        this.node_id, source_prov, destination
                    );
                    this.update_raw(source_prov, &destination).await
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

        provenance.update(&file, &process).await.unwrap();
        // Check that the process is now derived from the file
        assert_eq!(
            provenance.get_prov(&process).await,
            HashMap::from([(String::new(), HashSet::from([file, process]))])
        );
    }

    #[tokio::test]
    async fn unit_provenance_update_circular() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut provenance = ProvenanceService::default();
        let process = Resource::new_process_mock(0);
        let file = Resource::new_file("/tmp/test".to_string());

        provenance.update(&process, &file).await.unwrap();
        provenance.update(&file, &process).await.unwrap();

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

        provenance
            .update_raw(
                HashMap::from([
                    ("10.0.0.1".to_string(), HashSet::from([process0.clone()])),
                    ("10.0.0.2".to_string(), HashSet::from([process0.clone()])),
                ]),
                &process0,
            )
            .await
            .unwrap();
        provenance
            .update_raw(
                HashMap::from([
                    ("10.0.0.1".to_string(), HashSet::from([process1.clone()])),
                    ("10.0.0.2".to_string(), HashSet::from([file0.clone(), process1.clone()])),
                ]),
                &process0,
            )
            .await
            .unwrap();

        assert_eq!(
            provenance.get_prov(&process0).await,
            HashMap::from([
                (String::new(), HashSet::from([process0.clone()])),
                ("10.0.0.1".to_string(), HashSet::from([process0.clone(), process1.clone()])),
                ("10.0.0.2".to_string(), HashSet::from([file0, process0, process1]))
            ])
        );
    }

    #[tokio::test]
    async fn unit_provenance_tracks_propagation_nodes_for_local_sources() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut provenance = ProvenanceService::default().with_mode(Mode::Push);
        let process = Resource::new_process_mock(0);
        let stream = Resource::new_stream("10.0.0.1:8080".to_string(), "10.0.0.2:8081".to_string());

        // Process writes to a stream → record local node IP for the source
        provenance.update(&process, &stream).await.unwrap();

        assert_eq!(
            provenance.propagation.get(&process).unwrap().to_owned(),
            HashSet::from(["10.0.0.2".to_string()])
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
            ProvenanceResponse::Provenance(HashMap::from([(
                String::new(),
                HashSet::from([process.clone()])
            )]))
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
            ProvenanceResponse::Provenance(HashMap::from([(
                String::new(),
                HashSet::from([file, process])
            )]))
        );
    }
}
