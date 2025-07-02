use crate::traceability::{
    error::TraceabilityError,
    message::{P2mRequest, TraceabilityRequest, TraceabilityResponse},
};
use std::{
    collections::HashMap, future::Future, pin::Pin, sync::Arc, task::Poll, time::SystemTime,
};
use tokio::sync::Mutex;
use tower::Service;

use super::{
    message::P2mResponse,
    naming::{Identifier, Resource},
};

#[derive(Debug, Clone)]
pub struct P2mApiService<T> {
    node_id: String,
    resource_map: Arc<Mutex<HashMap<(i32, i32), (Identifier, Identifier)>>>,
    flow_map: Arc<Mutex<HashMap<u128, (Identifier, Identifier, bool)>>>,
    inner: T,
}

impl<T> P2mApiService<T> {
    pub fn new(inner: T) -> Self {
        Self::new_with_node_id(
            rustix::system::uname()
                .nodename()
                .to_str()
                .unwrap()
                .to_string()
                .to_lowercase(),
            inner,
        )
    }

    pub fn new_with_node_id(node_id: String, inner: T) -> Self {
        Self {
            node_id,
            resource_map: Arc::new(Mutex::new(HashMap::new())),
            flow_map: Arc::new(Mutex::new(HashMap::new())),
            inner,
        }
    }
}

impl<T> Service<P2mRequest> for P2mApiService<T>
where
    T: Service<TraceabilityRequest, Response = TraceabilityResponse, Error = TraceabilityError>
        + Clone
        + Send
        + 'static,
    T::Future: Send,
{
    type Response = P2mResponse;
    type Error = T::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: P2mRequest) -> Self::Future {
        let this = self.clone();
        let mut inner = std::mem::replace(&mut self.inner, this.inner.clone());
        Box::pin(async move {
            match request.clone() {
                P2mRequest::LocalEnroll { pid, fd, path } => {
                    let process = Identifier::new(this.node_id.clone(), Resource::new_process(pid));
                    let file = Identifier::new(this.node_id.clone(), Resource::new_file(path));
                    this.resource_map
                        .lock()
                        .await
                        .insert((pid, fd), (process.clone(), file.clone()));
                    Ok(P2mResponse::Ack)
                }
                P2mRequest::RemoteEnroll {
                    pid,
                    fd,
                    local_socket,
                    peer_socket,
                } => {
                    let process = Identifier::new(this.node_id.clone(), Resource::new_process(pid));
                    let stream = Identifier::new(
                        this.node_id.clone(),
                        Resource::new_stream(local_socket, peer_socket),
                    );
                    this.resource_map
                        .lock()
                        .await
                        .insert((pid, fd), (process.clone(), stream.clone()));
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

                        let (source, destination) = if output {
                            (process.clone(), fd.clone())
                        } else {
                            (fd.clone(), process.clone())
                        };
                        match inner
                            .call(TraceabilityRequest::Request {
                                source,
                                destination,
                            })
                            .await
                        {
                            Ok(TraceabilityResponse::Grant) => Ok(P2mResponse::Grant(flow_id)),
                            Err(e) => Err(e),
                            _ => Err(TraceabilityError::InternalTrace2eError),
                        }
                    } else {
                        Err(TraceabilityError::UndeclaredResource(pid, fd))
                    }
                }
                P2mRequest::IoReport { grant_id, .. } => {
                    let mut flow_map = this.flow_map.lock().await;
                    if let Some((process, fd, output)) = flow_map.remove(&grant_id) {
                        let (source, destination) = if output {
                            (process.clone(), fd.clone())
                        } else {
                            (fd.clone(), process.clone())
                        };
                        match inner
                            .call(TraceabilityRequest::Report {
                                source,
                                destination,
                                success: true,
                            })
                            .await
                        {
                            Ok(TraceabilityResponse::Ack) => Ok(P2mResponse::Ack),
                            Err(e) => Err(e),
                            _ => Err(TraceabilityError::InternalTrace2eError),
                        }
                    } else {
                        Err(TraceabilityError::NotFoundFlow(grant_id))
                    }
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use tower::{Service, ServiceBuilder, filter::FilterLayer, layer::layer_fn};

    use crate::traceability::{
        layers::{
            mock_compliance::TraceabilityMockService,
            provenance::ProvenanceService,
            sequencer::{SequencerService, WaitingQueueService},
        },
        validation::ResourceValidator,
    };

    use super::*;

    #[tokio::test]
    async fn unit_trace2e_service_request_response() {
        let mut trace2e_service = ServiceBuilder::new()
            .layer(layer_fn(|inner| P2mApiService::new(inner)))
            .layer(layer_fn(|inner| WaitingQueueService::new(inner, None)))
            .layer(layer_fn(|inner| SequencerService::new(inner)))
            .layer(layer_fn(|inner| ProvenanceService::new(inner)))
            .service(TraceabilityMockService::default());

        assert_eq!(
            trace2e_service
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
            trace2e_service
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

        let P2mResponse::Grant(flow_id) = trace2e_service
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
            trace2e_service
                .call(P2mRequest::IoReport {
                    pid: 1,
                    fd: 3,
                    grant_id: flow_id,
                    result: true
                })
                .await
                .unwrap(),
            P2mResponse::Ack
        );

        let P2mResponse::Grant(flow_id) = trace2e_service
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
            trace2e_service
                .call(P2mRequest::IoReport {
                    pid: 1,
                    fd: 3,
                    grant_id: flow_id,
                    result: true
                })
                .await
                .unwrap(),
            P2mResponse::Ack
        );
    }

    #[tokio::test]
    async fn unit_trace2e_service_validated_resources() {
        let mut trace2e_service = ServiceBuilder::new()
            .layer(FilterLayer::new(ResourceValidator::default()))
            .layer(layer_fn(|inner| P2mApiService::new(inner)))
            .layer(layer_fn(|inner| WaitingQueueService::new(inner, None)))
            .layer(layer_fn(|inner| SequencerService::new(inner)))
            .layer(layer_fn(|inner| ProvenanceService::new(inner)))
            .service(TraceabilityMockService::default());

        // Test with invalid process
        // This request is supposed to be filtered out by the validator
        assert_eq!(
            trace2e_service
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
            trace2e_service
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
    async fn unit_trace2e_service_io_invalid_request() {
        let mut trace2e_service = ServiceBuilder::new()
            .layer(layer_fn(|inner| P2mApiService::new(inner)))
            .layer(layer_fn(|inner| WaitingQueueService::new(inner, None)))
            .layer(layer_fn(|inner| SequencerService::new(inner)))
            .layer(layer_fn(|inner| ProvenanceService::new(inner)))
            .service(TraceabilityMockService::default());

        // Neither process nor fd are enrolled
        assert_eq!(
            trace2e_service
                .call(P2mRequest::IoRequest {
                    pid: std::process::id() as i32,
                    fd: 3,
                    output: true,
                })
                .await
                .unwrap_err(),
            TraceabilityError::UndeclaredResource(std::process::id() as i32, 3)
        );

        trace2e_service
            .call(P2mRequest::LocalEnroll {
                pid: std::process::id() as i32,
                fd: 4,
                path: "/tmp/test.txt".to_string(),
            })
            .await
            .unwrap();

        // Only process is enrolled
        assert_eq!(
            trace2e_service
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
    async fn unit_trace2e_service_io_invalid_report() {
        let mut trace2e_service = ServiceBuilder::new()
            .layer(layer_fn(|inner| P2mApiService::new(inner)))
            .layer(layer_fn(|inner| WaitingQueueService::new(inner, None)))
            .layer(layer_fn(|inner| SequencerService::new(inner)))
            .layer(layer_fn(|inner| ProvenanceService::new(inner)))
            .service(TraceabilityMockService::default());

        // Invalid grant id
        assert_eq!(
            trace2e_service
                .call(P2mRequest::IoReport {
                    pid: 1,
                    fd: 3,
                    grant_id: 0,
                    result: true,
                })
                .await
                .unwrap_err(),
            TraceabilityError::NotFoundFlow(0)
        );
    }
}
