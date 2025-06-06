use tokio::sync::Mutex;
use tower::Service;

use crate::traceability::{
    error::TraceabilityError,
    message::{TraceabilityRequest, TraceabilityResponse},
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
pub struct ProvenanceService<T> {
    inner: T,
    derived_from_map: Arc<Mutex<HashMap<Identifier, HashSet<Identifier>>>>,
}

impl<T> ProvenanceService<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            derived_from_map: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn get_prov(&self, id: Identifier) -> HashSet<Identifier> {
        self.derived_from_map
            .lock()
            .await
            .get(&id)
            .cloned()
            .unwrap_or(init_provenance(id))
    }

    async fn update(&mut self, source: Identifier, destination: Identifier) {
        let mut derived_from_map = self.derived_from_map.lock().await;
        let source_prov = derived_from_map
            .get(&source)
            .cloned()
            .unwrap_or(init_provenance(source.clone()));
        let destination_prov = derived_from_map
            .get(&destination)
            .cloned()
            .unwrap_or(init_provenance(destination.clone()));
        derived_from_map.insert(
            destination,
            destination_prov.union(&source_prov).cloned().collect(),
        );
    }
}

impl<T> Service<TraceabilityRequest> for ProvenanceService<T>
where
    T: Service<TraceabilityRequest, Response = TraceabilityResponse, Error = TraceabilityError>
        + Clone
        + Sync
        + Send
        + 'static,
    T::Future: Send,
{
    type Response = T::Response;
    type Error = T::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: TraceabilityRequest) -> Self::Future {
        let inner_clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, inner_clone);
        let mut this = self.clone();
        Box::pin(async move {
            match req.clone() {
                TraceabilityRequest::Request { source, .. } => {
                    // dummy provenance read
                    let _ = this.get_prov(source.clone()).await;
                    // todo!() // compliance check
                    inner.call(req).await
                }
                TraceabilityRequest::Report {
                    source,
                    destination,
                    success,
                } => {
                    if success {
                        this.update(source, destination).await;
                    }
                    inner.call(req).await
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use tower::{ServiceBuilder, layer::layer_fn};

    use crate::traceability::{layers::mock::TraceabilityMockService, naming::Resource};

    use super::*;

    #[tokio::test]
    async fn unit_provenance_update_simple() {
        let mut provenance = ProvenanceService::new(());
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
        let mut provenance = ProvenanceService::new(());
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
        let mut provenance = ServiceBuilder::new()
            .layer(layer_fn(|inner| ProvenanceService::new(inner)))
            .service(TraceabilityMockService::default());
        let node_id = "test".to_string();
        let process = Identifier::new(node_id.clone(), Resource::new_process(0));
        let file = Identifier::new(node_id.clone(), Resource::new_file("/tmp/test".to_string()));

        assert_eq!(
            provenance
                .call(TraceabilityRequest::Request {
                    source: file.clone(),
                    destination: process.clone(),
                })
                .await
                .unwrap(),
            TraceabilityResponse::Grant
        );

        assert_eq!(
            provenance
                .call(TraceabilityRequest::Report {
                    source: file.clone(),
                    destination: process.clone(),
                    success: true,
                })
                .await
                .unwrap(),
            TraceabilityResponse::Ack
        );

        assert_eq!(
            provenance.get_prov(process.clone()).await,
            HashSet::from([file.clone(), process.clone()])
        );
    }

    #[tokio::test]
    async fn unit_provenance_service_sequential_consistency() {
        let mut provenance = ServiceBuilder::new()
            .layer(layer_fn(|inner| ProvenanceService::new(inner)))
            .service(TraceabilityMockService::default());
        let node_id = "test".to_string();
        let process = Identifier::new(node_id.clone(), Resource::new_process(0));
        let file1 = Identifier::new(
            node_id.clone(),
            Resource::new_file("/tmp/test1".to_string()),
        );
        let file2 = Identifier::new(
            node_id.clone(),
            Resource::new_file("/tmp/test2".to_string()),
        );

        // file1 -> process
        assert_eq!(
            provenance
                .call(TraceabilityRequest::Request {
                    source: file1.clone(),
                    destination: process.clone(),
                })
                .await
                .unwrap(),
            TraceabilityResponse::Grant
        );

        // This is possible only because there is no sequencer layer
        // process -> file2
        assert_eq!(
            provenance
                .call(TraceabilityRequest::Request {
                    source: process.clone(),
                    destination: file2.clone(),
                })
                .await
                .unwrap(),
            TraceabilityResponse::Grant
        );

        assert_eq!(
            provenance
                .call(TraceabilityRequest::Report {
                    source: process.clone(),
                    destination: file2.clone(),
                    success: true,
                })
                .await
                .unwrap(),
            TraceabilityResponse::Ack
        );

        assert_eq!(
            provenance
                .call(TraceabilityRequest::Report {
                    source: file1.clone(),
                    destination: process.clone(),
                    success: true,
                })
                .await
                .unwrap(),
            TraceabilityResponse::Ack
        );

        // This is possible only because there is no sequencer layer.
        // The sequencer would force the order of the requests, so that the provenance
        // of file2 would be [file2, process, file1] and the provenance of process
        // would be [process, file1]
        assert_eq!(
            provenance.get_prov(file2.clone()).await,
            HashSet::from([file2.clone(), process.clone()])
        );
    }
}
