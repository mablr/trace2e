use crate::traceability::{
    error::TraceabilityError,
    message::{P2mRequest, TraceabilityResponse},
};
use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc, task::Poll};
use tokio::sync::Mutex;
use tower::Service;

use super::{
    message::{P2mResponse, TraceabilityRequest},
    naming::{Identifier, Resource},
    trace2e::TracE2EService,
};

#[derive(Debug, Clone)]
pub struct TraceabilityApiService {
    node_id: String,
    fd_map: Arc<Mutex<HashMap<(i32, i32), Identifier>>>, // Resource to be replaced by Identifier later
    process_map: Arc<Mutex<HashMap<i32, Identifier>>>,
    flow_id_counter: Arc<Mutex<usize>>,
    flow_map: Arc<Mutex<HashMap<usize, (Identifier, Identifier, bool)>>>,
    trace2e: TracE2EService,
}

impl Default for TraceabilityApiService {
    fn default() -> Self {
        Self::new_with_node_id(
            rustix::system::uname()
                .nodename()
                .to_str()
                .unwrap()
                .to_string()
                .to_lowercase(),
        )
    }
}

impl TraceabilityApiService {
    pub fn new_with_node_id(node_id: String) -> Self {
        Self {
            node_id,
            fd_map: Arc::new(Mutex::new(HashMap::new())),
            process_map: Arc::new(Mutex::new(HashMap::new())),
            flow_id_counter: Arc::new(Mutex::new(0)),
            flow_map: Arc::new(Mutex::new(HashMap::new())),
            trace2e: TracE2EService::default(),
        }
    }
}

impl Service<P2mRequest> for TraceabilityApiService {
    type Response = P2mResponse;
    type Error = TraceabilityError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.trace2e.poll_ready(cx)
    }

    fn call(&mut self, request: P2mRequest) -> Self::Future {
        let this = self.clone();
        let trace2e_clone = self.trace2e.clone();
        let mut trace2e = std::mem::replace(&mut self.trace2e, trace2e_clone);
        Box::pin(async move {
            match request {
                P2mRequest::LocalEnroll { pid, fd, path } => {
                    this.process_map.lock().await.insert(
                        pid,
                        Identifier::new(this.node_id.clone(), Resource::new_process(pid)),
                    );
                    this.fd_map.lock().await.insert(
                        (pid, fd),
                        Identifier::new(this.node_id.clone(), Resource::new_file(path)),
                    );
                    match trace2e
                        .call(TraceabilityRequest::InitResource(Identifier::new(
                            this.node_id.clone(),
                            Resource::new_process(pid),
                        )))
                        .await
                    {
                        Ok(TraceabilityResponse::Ack) => Ok(P2mResponse::Ack),
                        Err(e) => Err(e),
                        _ => unreachable!(),
                    }
                }
                P2mRequest::RemoteEnroll {
                    pid,
                    fd,
                    local_socket,
                    peer_socket,
                } => {
                    this.process_map.lock().await.insert(
                        pid,
                        Identifier::new(this.node_id.clone(), Resource::new_process(pid)),
                    );
                    this.fd_map.lock().await.insert(
                        (pid, fd),
                        Identifier::new(
                            this.node_id.clone(),
                            Resource::new_stream(local_socket, peer_socket),
                        ),
                    );
                    match trace2e
                        .call(TraceabilityRequest::InitResource(Identifier::new(
                            this.node_id.clone(),
                            Resource::new_process(pid),
                        )))
                        .await
                    {
                        Ok(TraceabilityResponse::Ack) => Ok(P2mResponse::Ack),
                        Err(e) => Err(e),
                        _ => unreachable!(),
                    }
                }
                P2mRequest::IoRequest { pid, fd, output } => {
                    if let (Some(process), Some(fd)) = (
                        this.process_map.lock().await.get(&pid),
                        this.fd_map.lock().await.get(&(pid, fd)),
                    ) {
                        let mut flow_map = this.flow_map.lock().await;
                        let mut flow_id_counter = this.flow_id_counter.lock().await;
                        let flow_id = *flow_id_counter;
                        *flow_id_counter += 1;
                        flow_map.insert(flow_id, (process.clone(), fd.clone(), output));
                        match trace2e
                            .call(TraceabilityRequest::Request {
                                process: process.clone(),
                                fd: fd.clone(),
                                output,
                            })
                            .await
                        {
                            Ok(TraceabilityResponse::Grant) => Ok(P2mResponse::Grant(flow_id)),
                            Err(e) => Err(e),
                            _ => unreachable!(),
                        }
                    } else {
                        Err(TraceabilityError::UndeclaredResource(pid, fd))
                    }
                }
                P2mRequest::IoReport { id, success } => {
                    let mut flow_map = this.flow_map.lock().await;
                    if let Some((process, fd, output)) = flow_map.remove(&id) {
                        match trace2e
                            .call(TraceabilityRequest::Report {
                                process,
                                fd,
                                output,
                                success,
                            })
                            .await
                        {
                            Ok(TraceabilityResponse::Ack) => Ok(P2mResponse::Ack),
                            Err(e) => Err(e),
                            _ => unreachable!(),
                        }
                    } else {
                        Err(TraceabilityError::NotFoundFlow(id))
                    }
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use tower::{ServiceBuilder, filter::FilterLayer};

    use crate::traceability::validation::ResourceValidator;

    use super::*;

    #[tokio::test]
    async fn unit_traceability_api_request_response() {
        let traceability_api = TraceabilityApiService::new_with_node_id("test".to_string());
        let mut traceability_api_service = ServiceBuilder::new().service(traceability_api);

        assert_eq!(
            traceability_api_service
                .call(P2mRequest::LocalEnroll {
                    pid: 1,
                    fd: 3,
                    path: "/tmp/test.txt".to_string()
                })
                .await
                .unwrap(),
            P2mResponse::Ack
        );
        assert_eq!(
            traceability_api_service
                .call(P2mRequest::RemoteEnroll {
                    pid: 1,
                    fd: 3,
                    local_socket: "127.0.0.1:8080".to_string(),
                    peer_socket: "127.0.0.1:8081".to_string()
                })
                .await
                .unwrap(),
            P2mResponse::Ack
        );

        assert_eq!(
            traceability_api_service
                .call(P2mRequest::IoRequest {
                    pid: 1,
                    fd: 3,
                    output: true
                })
                .await
                .unwrap(),
            P2mResponse::Grant(0)
        );
        assert_eq!(
            traceability_api_service
                .call(P2mRequest::IoReport {
                    id: 0,
                    success: true
                })
                .await
                .unwrap(),
            P2mResponse::Ack
        );

        assert_eq!(
            traceability_api_service
                .call(P2mRequest::IoRequest {
                    pid: 1,
                    fd: 3,
                    output: false
                })
                .await
                .unwrap(),
            P2mResponse::Grant(1)
        );
        assert_eq!(
            traceability_api_service
                .call(P2mRequest::IoReport {
                    id: 1,
                    success: true
                })
                .await
                .unwrap(),
            P2mResponse::Ack
        );
    }

    #[tokio::test]
    async fn unit_traceability_api_validated_resources() {
        let traceability_api = TraceabilityApiService::new_with_node_id("test".to_string());
        let mut traceability_api_service = ServiceBuilder::new()
            .layer(FilterLayer::new(ResourceValidator::default()))
            .service(traceability_api);

        // Test with invalid process
        // This request is supposed to be filtered out by the validator
        assert_eq!(
            traceability_api_service
                .call(P2mRequest::LocalEnroll {
                    pid: 0,
                    fd: 3,
                    path: "/tmp/test.txt".to_string()
                })
                .await
                .unwrap_err()
                .to_string(),
            "Traceability error, process not found (pid: 0)"
        );

        // Test successful process instantiation with validation
        assert_eq!(
            traceability_api_service
                .call(P2mRequest::LocalEnroll {
                    pid: std::process::id() as i32,
                    fd: 3,
                    path: "/tmp/test.txt".to_string()
                })
                .await
                .unwrap(),
            P2mResponse::Ack
        );
    }

    #[tokio::test]
    async fn unit_traceability_api_io_invalid_request() {
        let traceability_api = TraceabilityApiService::new_with_node_id("test".to_string());
        let mut traceability_api_service = ServiceBuilder::new().service(traceability_api);

        // Neither process nor fd are enrolled
        assert_eq!(
            traceability_api_service
                .call(P2mRequest::IoRequest {
                    pid: std::process::id() as i32,
                    fd: 3,
                    output: true,
                })
                .await
                .unwrap_err(),
            TraceabilityError::UndeclaredResource(std::process::id() as i32, 3)
        );

        traceability_api_service
            .call(P2mRequest::LocalEnroll {
                pid: std::process::id() as i32,
                fd: 4,
                path: "/tmp/test.txt".to_string(),
            })
            .await
            .unwrap();

        // Only process is enrolled
        assert_eq!(
            traceability_api_service
                .call(P2mRequest::IoRequest {
                    pid: std::process::id() as i32,
                    fd: 3,
                    output: true,
                })
                .await
                .unwrap_err(),
            TraceabilityError::UndeclaredResource(std::process::id() as i32, 3)
        );
    }

    #[tokio::test]
    async fn unit_traceability_api_io_invalid_report() {
        let traceability_api = TraceabilityApiService::new_with_node_id("test".to_string());
        let mut traceability_api_service = ServiceBuilder::new().service(traceability_api);

        // Invalid grant id
        assert_eq!(
            traceability_api_service
                .call(P2mRequest::IoReport {
                    id: 0,
                    success: true,
                })
                .await
                .unwrap_err(),
            TraceabilityError::NotFoundFlow(0)
        );
    }
}
