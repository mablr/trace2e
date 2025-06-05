use crate::traceability::{
    error::TraceabilityError,
    message::{P2mRequest, TraceabilityResponse},
};
use std::{
    collections::HashMap, future::Future, pin::Pin, sync::Arc, task::Poll, time::SystemTime,
};
use tokio::sync::Mutex;
use tower::Service;

use super::{
    message::{P2mResponse, TraceabilityRequest},
    naming::{Identifier, Resource},
};

#[derive(Debug, Clone)]
pub struct TraceabilityApiService {
    node_id: String,
    resource_map: Arc<Mutex<HashMap<(i32, i32), (Identifier, Identifier)>>>,
    flow_map: Arc<Mutex<HashMap<u128, (Identifier, Identifier, bool)>>>,
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
            resource_map: Arc::new(Mutex::new(HashMap::new())),
            flow_map: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Service<P2mRequest> for TraceabilityApiService {
    type Response = P2mResponse;
    type Error = TraceabilityError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(
        &mut self,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: P2mRequest) -> Self::Future {
        let this = self.clone();
        Box::pin(async move {
            match request {
                P2mRequest::LocalEnroll { pid, fd, path } => {
                    this.resource_map.lock().await.insert(
                        (pid, fd),
                        (
                            Identifier::new(this.node_id.clone(), Resource::new_process(pid)),
                            Identifier::new(this.node_id.clone(), Resource::new_file(path)),
                        ),
                    );
                    Ok(P2mResponse::Ack)
                }
                P2mRequest::RemoteEnroll {
                    pid,
                    fd,
                    local_socket,
                    peer_socket,
                } => {
                    this.resource_map.lock().await.insert(
                        (pid, fd),
                        (
                            Identifier::new(this.node_id.clone(), Resource::new_process(pid)),
                            Identifier::new(
                                this.node_id.clone(),
                                Resource::new_stream(local_socket, peer_socket),
                            ),
                        ),
                    );
                    Ok(P2mResponse::Ack)
                }
                P2mRequest::IoRequest { pid, fd, output } => {
                    if let Some((process, fd)) = this.resource_map.lock().await.get(&(pid, fd)) {
                        let mut flow_map = this.flow_map.lock().await;

                        let flow_id = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)
                        {
                            Ok(n) => n.as_nanos(),
                            Err(_) => return Err(TraceabilityError::SystemTimeError),
                        };
                        flow_map.insert(flow_id, (process.clone(), fd.clone(), output));
                        Ok(P2mResponse::Grant(flow_id))
                    } else {
                        Err(TraceabilityError::UndeclaredResource(pid, fd))
                    }
                }
                P2mRequest::IoReport { id, success } => {
                    let mut flow_map = this.flow_map.lock().await;
                    if let Some((process, fd, output)) = flow_map.remove(&id) {
                        Ok(P2mResponse::Ack)
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

        let P2mResponse::Grant(flow_id) = traceability_api_service
            .call(P2mRequest::IoRequest {
                pid: 1,
                fd: 3,
                output: true,
            })
            .await
            .unwrap()
        else {
            panic!("Expected P2mResponse::Grant");
        };
        assert_eq!(
            traceability_api_service
                .call(P2mRequest::IoReport {
                    id: flow_id,
                    success: true
                })
                .await
                .unwrap(),
            P2mResponse::Ack
        );

        let P2mResponse::Grant(flow_id) = traceability_api_service
            .call(P2mRequest::IoRequest {
                pid: 1,
                fd: 3,
                output: false,
            })
            .await
            .unwrap()
        else {
            panic!("Expected P2mResponse::Grant");
        };
        assert_eq!(
            traceability_api_service
                .call(P2mRequest::IoReport {
                    id: flow_id,
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
