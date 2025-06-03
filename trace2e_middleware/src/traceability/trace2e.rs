use std::{collections::HashMap, pin::Pin, sync::Arc, task::Poll};

use tokio::sync::Mutex;
use tower::{Service, ServiceBuilder};

use super::{
    error::TraceabilityError,
    message::{ResourceRequest, ResourceResponse, TraceabilityRequest, TraceabilityResponse},
    naming::Identifier,
    provenance::Provenance,
    resource::GuardedResourceService,
};

#[derive(Debug, Default, Clone)]
pub struct TracE2EService {
    provenance: Arc<Mutex<Provenance>>,
    resources_map: Arc<Mutex<HashMap<Identifier, GuardedResourceService>>>,
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
                        id.clone(),
                        ServiceBuilder::new().service(GuardedResourceService::new()),
                    );
                    Ok(TraceabilityResponse::Ack)
                }
                TraceabilityRequest::Request {
                    process,
                    fd,
                    output,
                } => {
                    // Dummy implementation for now (No compliance enforcement, only resource reservation)
                    resources_map.lock().await.get_mut(&process).unwrap().call(if output {ResourceRequest::ReadRequest} else {ResourceRequest::WriteRequest}).await?;
                    resources_map.lock().await.get_mut(&fd).unwrap().call(if output {ResourceRequest::WriteRequest} else {ResourceRequest::ReadRequest}).await?;
                    Ok(TraceabilityResponse::Ack)
                }
                TraceabilityRequest::Report {
                    process,
                    fd,
                    success,
                    output,
                } => {
                    if success {
                        if output {
                            provenance.lock().await.update(process.clone(), fd.clone());
                        } else {
                            provenance.lock().await.update(fd.clone(), process.clone());
                        }
                        resources_map.lock().await.get_mut(&process).unwrap().call(if output {ResourceRequest::ReadReport} else {ResourceRequest::WriteReport}).await?;
                        resources_map.lock().await.get_mut(&fd).unwrap().call(if output {ResourceRequest::WriteReport} else {ResourceRequest::ReadReport}).await?;
                    } else {
                        resources_map.lock().await.get_mut(&process).unwrap().call(if output {ResourceRequest::ReleaseShared} else {ResourceRequest::ReleaseExclusive}).await?;
                        resources_map.lock().await.get_mut(&fd).unwrap().call(if output {ResourceRequest::ReleaseExclusive} else {ResourceRequest::ReleaseShared}).await?;
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

    #[tokio::test]
    async fn unit_trace2e_mediate_simple_output_to_file() {
        let node_id = "test".to_string();
        let process = Identifier::new(node_id.clone(), Resource::new_process(0));
        let file = Identifier::new(node_id.clone(), Resource::new_file("/tmp/test".to_string()));
        let mut trace2e_service = ServiceBuilder::new().service(TracE2EService::default());
        trace2e_service.call(TraceabilityRequest::InitResource(process.clone())).await.unwrap();
        trace2e_service.call(TraceabilityRequest::InitResource(file.clone())).await.unwrap();


        trace2e_service.call(TraceabilityRequest::Request {
            process: process.clone(),
            fd: file.clone(),
            output: true,
        }).await.unwrap();

        trace2e_service.call(TraceabilityRequest::Report {
            process: process.clone(),
            fd: file.clone(),
            success: true,
            output: true,
        }).await.unwrap();
    }

    #[tokio::test]
    async fn unit_trace2e_mediate_concurrent_outputs_to_files() {
        let node_id = "test".to_string();
        let process = Identifier::new(node_id.clone(), Resource::new_process(0));
        let file1 = Identifier::new(node_id.clone(), Resource::new_file("/tmp/test1".to_string()));
        let file2 = Identifier::new(node_id.clone(), Resource::new_file("/tmp/test2".to_string()));
        let mut trace2e_service = ServiceBuilder::new().service(TracE2EService::default());
        trace2e_service.call(TraceabilityRequest::InitResource(process.clone())).await.unwrap();
        trace2e_service.call(TraceabilityRequest::InitResource(file1.clone())).await.unwrap();
        trace2e_service.call(TraceabilityRequest::InitResource(process.clone())).await.unwrap();            
        trace2e_service.call(TraceabilityRequest::InitResource(file2.clone())).await.unwrap();

        trace2e_service.call(TraceabilityRequest::Request {
            process: process.clone(),
            fd: file1.clone(),
            output: true,
        }).await.unwrap();

        trace2e_service.call(TraceabilityRequest::Request {
            process: process.clone(),
            fd: file2.clone(),
            output: true,
        }).await.unwrap();

        trace2e_service.call(TraceabilityRequest::Report {
            process: process.clone(),
            fd: file1.clone(),
            success: true,
            output: true,
        }).await.unwrap();

        trace2e_service.call(TraceabilityRequest::Report {
            process: process.clone(),
            fd: file2.clone(),
            success: true,
            output: true,
        }).await.unwrap();

        println!("Provenance: {:?}", trace2e_service.provenance.lock().await);
    }
}