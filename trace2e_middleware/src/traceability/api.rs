use crate::traceability::{
    error::TraceabilityError,
    message::{P2mRequest, TraceabilityResponse},
};
use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc, task::Poll};
use tokio::sync::Mutex;
use tower::Service;

use super::{naming::{Identifier, Resource}};

#[derive(Debug, Clone)]
pub struct TraceabilityApiService {
    node_id: String,
    fd_map: Arc<Mutex<HashMap<(i32, i32), Resource>>>, // Resource to be replaced by Identifier later
    process_map: Arc<Mutex<HashMap<i32, Resource>>>,
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
        }
    }
}

impl Service<P2mRequest> for TraceabilityApiService {
    type Response = TraceabilityResponse;
    type Error = TraceabilityError;
    type Future = Pin<Box<dyn Future<Output = Result<TraceabilityResponse, TraceabilityError>>>>;

    fn poll_ready(
        &mut self,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(())) // TODO: Implement readiness check
    }

    fn call(&mut self, request: P2mRequest) -> Self::Future {
        let this = self.clone();
        Box::pin(async move {
            match request {
                P2mRequest::LocalEnroll { pid, fd, path } => {
                    this
                        .process_map
                        .lock()
                        .await
                        .insert(pid, Resource::new_process(pid));
                    this
                        .fd_map
                        .lock()
                        .await
                        .insert((pid, fd), Resource::new_file(path));
                    Ok(TraceabilityResponse::Ack)
                }
                P2mRequest::RemoteEnroll {
                    pid,
                    fd,
                    local_socket,
                    peer_socket,
                } => {
                    this
                        .process_map
                        .lock()
                        .await
                        .insert(pid, Resource::new_process(pid));
                    this
                        .fd_map
                        .lock()
                        .await
                        .insert((pid, fd), Resource::new_stream(local_socket, peer_socket));
                    Ok(TraceabilityResponse::Ack)
                }
                P2mRequest::IoRequest { pid, fd, output } => {
                    // Dummy implementation for now
                    if let (Some(process), Some(fd)) = (
                        this.process_map.lock().await.get(&pid),
                        this.fd_map.lock().await.get(&(pid, fd)),
                    ) {
                        // let flow = Flow::new(
                        //     Identifier {
                        //         node: this.node_id.clone(),
                        //         resource: process.clone(),
                        //     },
                        //     Identifier {
                        //         node: this.node_id.clone(),
                        //         resource: fd.clone(),
                        //     },
                        //     output,
                        // );
                        // let mut flow_map = this.flow_map.lock().await;
                        // let mut flow_id_counter = this.flow_id_counter.lock().await;
                        // let flow_id = *flow_id_counter;
                        // *flow_id_counter += 1;
                        // flow_map.insert(flow_id, flow);
                        Ok(TraceabilityResponse::Grant(0))
                    } else {
                        Err(TraceabilityError::UndeclaredResource(pid, fd))
                    }
                }
                P2mRequest::IoReport { id, success: _ } => {
                    // Dummy implementation for now
                    // let mut flow_map = this.flow_map.lock().await;
                    // if flow_map.remove(&id).is_some() {
                    //     Ok(TraceabilityResponse::Ack)
                    // } else {
                    //     Err(TraceabilityError::NotFoundFlow(id))
                    // }
                    todo!()
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
            TraceabilityResponse::Ack
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
            TraceabilityResponse::Ack
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
            TraceabilityResponse::Grant(0)
        );
        assert_eq!(
            traceability_api_service
                .call(P2mRequest::IoReport {
                    id: 0,
                    success: true
                })
                .await
                .unwrap(),
            TraceabilityResponse::Ack
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
            TraceabilityResponse::Grant(1)
        );
        assert_eq!(
            traceability_api_service
                .call(P2mRequest::IoReport {
                    id: 1,
                    success: true
                })
                .await
                .unwrap(),
            TraceabilityResponse::Ack
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
            TraceabilityResponse::Ack
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
