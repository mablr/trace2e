use std::{
    collections::{HashMap, HashSet},
    pin::Pin,
    sync::Arc,
    task::Poll,
};

use tokio::sync::Mutex;
use tower::{Service, ServiceBuilder, layer::layer_fn};

use super::{
    error::TraceabilityError,
    message::{TraceabilityRequest, TraceabilityResponse},
    naming::Identifier,
    reservation::{GuardedResource, ReservationService, WaitingQueueService},
    resource::ResourceService,
};

#[derive(Debug, Default, Clone)]
pub struct Provenance {
    derived_from_map: HashMap<Identifier, HashSet<Identifier>>,
}

impl Provenance {
    pub fn enroll(&mut self, id: Identifier) {
        if !self.derived_from_map.contains_key(&id) {
            let mut derived_from = HashSet::new();
            derived_from.insert(id.clone());
            self.derived_from_map.insert(id.clone(), derived_from);
        }
    }

    pub fn get_prov(&self, id: Identifier) -> Option<HashSet<Identifier>> {
        self.derived_from_map.get(&id).cloned()
    }

    pub fn get_prov_mut(&mut self, id: Identifier) -> Option<&mut HashSet<Identifier>> {
        self.derived_from_map.get_mut(&id)
    }

    pub fn update(&mut self, source: Identifier, destination: Identifier) {
        if let (Some(source_prov), Some(destination_prov)) =
            (self.get_prov(source), self.get_prov_mut(destination))
        {
            for id in source_prov {
                destination_prov.insert(id);
            }
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct TracE2EService {
    provenance: Arc<Mutex<Provenance>>,
    resources_map: Arc<
        Mutex<
            HashMap<
                Identifier,
                Arc<GuardedResource<ResourceService, WaitingQueueService<ReservationService>>>,
            >,
        >,
    >,
}

impl Service<TraceabilityRequest> for TracE2EService {
    type Response = TraceabilityResponse;
    type Error = TraceabilityError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: TraceabilityRequest) -> Self::Future {
        let provenance = self.provenance.clone();
        let resources_map = self.resources_map.clone();
        Box::pin(async move {
            match req {
                TraceabilityRequest::InitResource(id) => {
                    provenance.lock().await.enroll(id.clone());
                    resources_map.lock().await.insert(
                        id,
                        Arc::new(GuardedResource::new(
                            ServiceBuilder::new().service(ResourceService::default()),
                            ServiceBuilder::new()
                                .layer(layer_fn(|service| WaitingQueueService::new(service)))
                                .service(ReservationService::default()),
                        )),
                    );
                    Ok(TraceabilityResponse::Ack)
                }
                TraceabilityRequest::Request {
                    process,
                    fd,
                    output,
                } => {
                    // Dummy implementation for now (No compliance enforcement, only resource reservation)
                    todo!()
                }
                TraceabilityRequest::Report {
                    process,
                    fd,
                    success,
                    output,
                } => {
                    if success {
                        if output {
                            provenance.lock().await.update(process, fd);
                        } else {
                            provenance.lock().await.update(fd, process);
                        }
                    }
                    Ok(TraceabilityResponse::Ack)
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::traceability::naming::Resource;

    use super::*;

    #[test]
    fn unit_trace2e_service_provenance_update_simple() {
        let mut provenance = Provenance::default();
        let node_id = "test".to_string();
        let process = Identifier::new(node_id.clone(), Resource::new_process(0));
        let file = Identifier::new(node_id.clone(), Resource::new_file("/tmp/test".to_string()));

        provenance.enroll(process.clone());
        provenance.enroll(file.clone());

        assert_eq!(
            provenance.get_prov(file.clone()),
            Some(HashSet::from([file.clone()]))
        );
        assert_eq!(
            provenance.get_prov(process.clone()),
            Some(HashSet::from([process.clone()]))
        );

        provenance.update(process.clone(), file.clone());

        // Check that the file is now derived from the process
        assert!(
            provenance
                .get_prov(file.clone())
                .unwrap()
                .is_superset(&provenance.get_prov(process.clone()).unwrap())
        );
    }

    #[test]
    fn unit_trace2e_service_provenance_update_circular() {
        let mut provenance = Provenance::default();
        let node_id = "test".to_string();
        let process = Identifier::new(node_id.clone(), Resource::new_process(0));
        let file = Identifier::new(node_id.clone(), Resource::new_file("/tmp/test".to_string()));

        provenance.enroll(process.clone());
        provenance.enroll(file.clone());

        provenance.update(process.clone(), file.clone());
        provenance.update(file.clone(), process.clone());

        // Check the proper handling of circular dependencies
        assert_eq!(
            provenance.get_prov(file.clone()).unwrap(),
            provenance.get_prov(process.clone()).unwrap()
        );
    }
}
