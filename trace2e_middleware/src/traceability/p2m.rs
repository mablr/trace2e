use crate::traceability::{
    api::{
        ComplianceRequest, ComplianceResponse, P2mRequest, P2mResponse, ProvenanceRequest,
        ProvenanceResponse, SequencerRequest, SequencerResponse,
    },
    error::TraceabilityError,
    naming::{Identifier, Resource},
};
use std::{
    collections::HashMap, future::Future, pin::Pin, sync::Arc, task::Poll, time::SystemTime,
};
use tokio::sync::Mutex;
use tower::Service;

type ResourceMap = HashMap<(i32, i32), (Identifier, Identifier)>;
type FlowMap = HashMap<u128, (Identifier, Identifier, bool)>;

#[derive(Debug, Clone)]
pub struct P2mApiService<S, P, C> {
    node_id: String,
    resource_map: Arc<Mutex<ResourceMap>>,
    flow_map: Arc<Mutex<FlowMap>>,
    sequencer: S,
    provenance: P,
    compliance: C,
}

impl<S, P, C> P2mApiService<S, P, C> {
    pub fn new(sequencer: S, provenance: P, compliance: C) -> Self {
        Self::new_with_node_id(String::new(), sequencer, provenance, compliance)
    }

    pub fn new_with_node_id(node_id: String, sequencer: S, provenance: P, compliance: C) -> Self {
        Self {
            node_id,
            resource_map: Arc::new(Mutex::new(HashMap::new())),
            flow_map: Arc::new(Mutex::new(HashMap::new())),
            sequencer,
            provenance,
            compliance,
        }
    }
}

impl<S, P, C> Service<P2mRequest> for P2mApiService<S, P, C>
where
    S: Service<SequencerRequest, Response = SequencerResponse, Error = TraceabilityError>
        + Clone
        + Send
        + 'static,
    S::Future: Send,
    P: Service<ProvenanceRequest, Response = ProvenanceResponse, Error = TraceabilityError>
        + Clone
        + Send
        + 'static,
    P::Future: Send,
    C: Service<ComplianceRequest, Response = ComplianceResponse, Error = TraceabilityError>
        + Clone
        + Send
        + 'static,
    C::Future: Send,
{
    type Response = P2mResponse;
    type Error = TraceabilityError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: P2mRequest) -> Self::Future {
        let this = self.clone();
        let mut sequencer = std::mem::replace(&mut self.sequencer, this.sequencer.clone());
        let mut provenance = std::mem::replace(&mut self.provenance, this.provenance.clone());
        let mut compliance = std::mem::replace(&mut self.compliance, this.compliance.clone());
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
                        match sequencer
                            .call(SequencerRequest::ReserveFlow {
                                source: source.clone(),
                                destination: destination.clone(),
                            })
                            .await
                        {
                            Ok(SequencerResponse::FlowReserved) => {
                                match (
                                    provenance
                                        .call(ProvenanceRequest::GetProvenance {
                                            id: source.clone(),
                                        })
                                        .await,
                                    provenance
                                        .call(ProvenanceRequest::GetProvenance {
                                            id: destination.clone(),
                                        })
                                        .await,
                                ) {
                                    (
                                        Ok(ProvenanceResponse::Provenance {
                                            derived_from: _source_prov,
                                        }),
                                        Ok(ProvenanceResponse::Provenance {
                                            derived_from: _destination_prov,
                                        }),
                                    ) => {
                                        // Todo: use source_prov and destination_prov to check compliance
                                        match compliance
                                            .call(ComplianceRequest::CheckCompliance {
                                                source,
                                                destination,
                                            })
                                            .await
                                        {
                                            Ok(ComplianceResponse::Grant) => {
                                                Ok(P2mResponse::Grant(flow_id))
                                            }
                                            Err(e) => Err(e),
                                            _ => Err(TraceabilityError::InternalTrace2eError),
                                        }
                                    }
                                    _ => Err(TraceabilityError::InternalTrace2eError),
                                }
                            }
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
                        provenance
                            .call(ProvenanceRequest::UpdateProvenance {
                                source: source.clone(),
                                destination: destination.clone(),
                            })
                            .await?;
                        sequencer
                            .call(SequencerRequest::ReleaseFlow {
                                source: source.clone(),
                                destination: destination.clone(),
                            })
                            .await?;
                        Ok(P2mResponse::Ack)
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
            compliance::ComplianceService,
            provenance::ProvenanceService,
            sequencer::{SequencerService, WaitingQueueService},
        },
        validation::ResourceValidator,
    };

    use super::*;

    #[tokio::test]
    async fn unit_trace2e_service_request_response() {
        let sequencer = ServiceBuilder::new()
            .layer(layer_fn(|inner| WaitingQueueService::new(inner, None)))
            .service(SequencerService::default());
        let provenance = ServiceBuilder::new().service(ProvenanceService::default());
        let compliance = ServiceBuilder::new().service(ComplianceService::default());
        let mut p2m_service =
            ServiceBuilder::new().service(P2mApiService::new(sequencer, provenance, compliance));

        assert_eq!(
            p2m_service
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
            p2m_service
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

        let P2mResponse::Grant(flow_id) = p2m_service
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
            p2m_service
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

        let P2mResponse::Grant(flow_id) = p2m_service
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
            p2m_service
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
        let sequencer = ServiceBuilder::new()
            .layer(layer_fn(|inner| WaitingQueueService::new(inner, None)))
            .service(SequencerService::default());
        let provenance = ServiceBuilder::new().service(ProvenanceService::default());
        let compliance = ServiceBuilder::new().service(ComplianceService::default());
        let mut p2m_service = ServiceBuilder::new()
            .layer(FilterLayer::new(ResourceValidator::default()))
            .service(P2mApiService::new(sequencer, provenance, compliance));

        // Test with invalid process
        // This request is supposed to be filtered out by the validator
        assert_eq!(
            p2m_service
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
            p2m_service
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
        let sequencer = ServiceBuilder::new()
            .layer(layer_fn(|inner| WaitingQueueService::new(inner, None)))
            .service(SequencerService::default());
        let provenance = ServiceBuilder::new().service(ProvenanceService::default());
        let compliance = ServiceBuilder::new().service(ComplianceService::default());
        let mut p2m_service = ServiceBuilder::new()
            .layer(FilterLayer::new(ResourceValidator::default()))
            .service(P2mApiService::new(sequencer, provenance, compliance));

        // Neither process nor fd are enrolled
        assert_eq!(
            p2m_service
                .call(P2mRequest::IoRequest {
                    pid: std::process::id() as i32,
                    fd: 3,
                    output: true,
                })
                .await
                .unwrap_err()
                .to_string(),
            format!(
                "Traceability error, undeclared resource (pid: {}, fd: 3)",
                std::process::id() as i32
            )
        );

        p2m_service
            .call(P2mRequest::LocalEnroll {
                pid: std::process::id() as i32,
                fd: 4,
                path: "/tmp/test.txt".to_string(),
            })
            .await
            .unwrap();

        // Only process is enrolled
        assert_eq!(
            p2m_service
                .call(P2mRequest::IoRequest {
                    pid: std::process::id() as i32,
                    fd: 3,
                    output: true,
                })
                .await
                .unwrap_err()
                .to_string(),
            format!(
                "Traceability error, undeclared resource (pid: {}, fd: 3)",
                std::process::id() as i32
            )
        );
    }

    #[tokio::test]
    async fn unit_trace2e_service_io_invalid_report() {
        let sequencer = ServiceBuilder::new()
            .layer(layer_fn(|inner| WaitingQueueService::new(inner, None)))
            .service(SequencerService::default());
        let provenance = ServiceBuilder::new().service(ProvenanceService::default());
        let compliance = ServiceBuilder::new().service(ComplianceService::default());
        let mut p2m_service = ServiceBuilder::new()
            .layer(FilterLayer::new(ResourceValidator::default()))
            .service(P2mApiService::new(sequencer, provenance, compliance));

        // Invalid grant id
        assert_eq!(
            p2m_service
                .call(P2mRequest::IoReport {
                    pid: 1,
                    fd: 3,
                    grant_id: 0,
                    result: true,
                })
                .await
                .unwrap_err()
                .to_string(),
            "Traceability error, flow not found (id: 0)"
        );
    }
}
