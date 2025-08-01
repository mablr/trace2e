use tokio::sync::Mutex;
use tower::Service;

use crate::traceability::{
    api::{ProvenanceRequest, ProvenanceResponse},
    error::TraceabilityError,
    naming::Resource,
};
use std::{
    collections::{HashMap, HashSet},
    pin::Pin,
    sync::Arc,
    task::Poll,
};

type ProvenanceMap = HashMap<Resource, HashMap<String, HashSet<Resource>>>;

#[derive(Debug, Default, Clone)]
pub struct ProvenanceService {
    provenance: Arc<Mutex<ProvenanceMap>>,
}

impl ProvenanceService {
    fn init_provenance(resource: Resource) -> HashMap<String, HashSet<Resource>> {
        if resource.is_file() || resource.is_process() {
            HashMap::from([(String::new(), HashSet::from([resource]))])
        } else {
            // Streams have no impact on provenance
            HashMap::new()
        }
    }

    /// Get the provenance of a resource
    ///
    /// This function returns a map of node IDs to the provenance of the resource for that node.
    /// If the resource is not found, it returns an empty map.
    async fn get_prov(&self, resource: Resource) -> HashMap<String, HashSet<Resource>> {
        let provenance = self.provenance.lock().await;
        provenance
            .get(&resource)
            .unwrap_or(&Self::init_provenance(resource))
            .clone()
    }

    /// Set the provenance of a resource
    async fn set_prov(&mut self, resource: Resource, prov: HashMap<String, HashSet<Resource>>) {
        self.provenance.lock().await.insert(resource, prov);
    }

    /// Get the local references of a resource
    pub async fn get_local_references(&self, resource: Resource) -> HashSet<Resource> {
        self.get_prov(resource)
            .await
            .get(&String::new())
            .cloned()
            .unwrap_or_default()
    }

    /// Get the remote references of a resource
    pub async fn get_remote_references(
        &self,
        resource: Resource,
    ) -> HashMap<String, HashSet<Resource>> {
        let mut prov = self.get_prov(resource).await;
        prov.remove(&String::new()); // Remove the local references
        prov
    }

    /// Update the provenance of the destination with the source
    ///
    /// Note that this function does not guarantee sequential consistency,
    /// this is the role of the sequencer.
    async fn update(&mut self, source: Resource, destination: Resource) -> ProvenanceResponse {
        let source_prov = self.get_prov(source.clone()).await;
        self.update_raw(source_prov, destination).await
    }

    /// Update the provenance of the destination with the raw source provenance
    ///
    /// Note that this function does not guarantee sequential consistency,
    /// this is the role of the sequencer.
    async fn update_raw(
        &mut self,
        source_prov: HashMap<String, HashSet<Resource>>,
        destination: Resource,
    ) -> ProvenanceResponse {
        let mut updated = false;
        let mut destination_prov = self.get_prov(destination.clone()).await;
        for (node_id, node_source_prov) in source_prov.clone() {
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
            self.set_prov(destination, destination_prov).await;
            ProvenanceResponse::ProvenanceUpdated
        } else {
            ProvenanceResponse::ProvenanceNotUpdated
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
                ProvenanceRequest::GetLocalReferences(resource) => {
                    Ok(ProvenanceResponse::LocalReferences(
                        this.get_local_references(resource.clone()).await,
                    ))
                }
                ProvenanceRequest::GetRemoteReferences(resource) => {
                    Ok(ProvenanceResponse::RemoteReferences(
                        this.get_remote_references(resource.clone()).await,
                    ))
                }
                ProvenanceRequest::UpdateProvenance {
                    source,
                    destination,
                } => Ok(this.update(source, destination).await),
                ProvenanceRequest::UpdateProvenanceRaw {
                    source_prov,
                    destination,
                } => Ok(this.update_raw(source_prov, destination).await),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unit_provenance_update_simple() {
        let mut provenance = ProvenanceService::default();
        let process = Resource::new_process(0);
        let file = Resource::new_file("/tmp/test".to_string());

        provenance.update(file.clone(), process.clone()).await;
        // Check that the process is now derived from the file
        assert_eq!(
            provenance.get_local_references(process.clone()).await,
            HashSet::from([file.clone(), process.clone()])
        );
    }

    #[tokio::test]
    async fn unit_provenance_update_circular() {
        let mut provenance = ProvenanceService::default();
        let process = Resource::new_process(0);
        let file = Resource::new_file("/tmp/test".to_string());

        provenance.update(process.clone(), file.clone()).await;
        provenance.update(file.clone(), process.clone()).await;

        // Check the proper handling of circular dependencies
        assert_eq!(
            provenance.get_local_references(file.clone()).await,
            provenance.get_local_references(process.clone()).await
        );
    }

    #[tokio::test]
    async fn unit_provenance_update_stream_idempotent() {
        let mut provenance = ProvenanceService::default();
        let process0 = Resource::new_process(0);
        let process1 = Resource::new_process(1);

        let stream =
            Resource::new_stream("127.0.0.1:8080".to_string(), "127.0.0.1:8081".to_string());

        provenance.update(process0.clone(), stream.clone()).await;
        provenance.update(stream.clone(), process1.clone()).await;

        assert_eq!(
            provenance.get_local_references(process1.clone()).await,
            HashSet::from([process0.clone(), process1.clone()]) // Streams have no impact on provenance
        );
    }

    #[tokio::test]
    async fn unit_provenance_update_multiple_nodes() {
        let mut provenance = ProvenanceService::default();
        let process0 = Resource::new_process(0);
        let process1 = Resource::new_process(1);
        let file0 = Resource::new_file("/tmp/test0".to_string());

        provenance
            .update_raw(
                HashMap::from([
                    ("10.0.0.1".to_string(), HashSet::from([process0.clone()])),
                    ("10.0.0.2".to_string(), HashSet::from([process0.clone()])),
                ]),
                process0.clone(),
            )
            .await;
        provenance
            .update_raw(
                HashMap::from([
                    ("10.0.0.1".to_string(), HashSet::from([process1.clone()])),
                    (
                        "10.0.0.2".to_string(),
                        HashSet::from([file0.clone(), process1.clone()]),
                    ),
                ]),
                process0.clone(),
            )
            .await;

        assert_eq!(
            provenance.get_remote_references(process0.clone()).await,
            HashMap::from([
                (
                    "10.0.0.1".to_string(),
                    HashSet::from([process0.clone(), process1.clone()])
                ),
                (
                    "10.0.0.2".to_string(),
                    HashSet::from([file0.clone(), process0.clone(), process1.clone()])
                )
            ])
        );

        assert_eq!(
            provenance.get_local_references(process0.clone()).await,
            HashSet::from([process0.clone()])
        );

        assert_eq!(
            provenance.get_prov(process0.clone()).await,
            HashMap::from([
                (String::new(), HashSet::from([process0.clone()])),
                (
                    "10.0.0.1".to_string(),
                    HashSet::from([process0.clone(), process1.clone()])
                ),
                (
                    "10.0.0.2".to_string(),
                    HashSet::from([file0.clone(), process0.clone(), process1.clone()])
                )
            ])
        );
    }

    #[tokio::test]
    async fn unit_provenance_service_flow_simple() {
        let mut provenance = ProvenanceService::default();
        let process = Resource::new_process(0);
        let file = Resource::new_file("/tmp/test".to_string());

        assert_eq!(
            provenance
                .call(ProvenanceRequest::GetLocalReferences(process.clone()))
                .await
                .unwrap(),
            ProvenanceResponse::LocalReferences(HashSet::from([process.clone()]))
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
                .call(ProvenanceRequest::GetLocalReferences(process.clone()))
                .await
                .unwrap(),
            ProvenanceResponse::LocalReferences(HashSet::from([file.clone(), process.clone()]))
        );
    }
}
