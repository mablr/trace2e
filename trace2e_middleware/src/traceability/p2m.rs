use crate::traceability::{
    api::{
        ComplianceRequest, ComplianceResponse, M2mRequest, M2mResponse, P2mRequest, P2mResponse,
        ProvenanceRequest, ProvenanceResponse, SequencerRequest, SequencerResponse,
    },
    error::TraceabilityError,
    naming::{Identifier, Resource},
};
use std::{
    collections::{HashMap, HashSet},
    future::Future,
    pin::Pin,
    sync::Arc,
    task::Poll,
    time::SystemTime,
};
use tokio::sync::Mutex;
use tower::Service;

type ResourceMap = HashMap<(i32, i32), (Identifier, Identifier)>;
type FlowMap = HashMap<u128, (Identifier, Identifier, bool)>;

#[derive(Debug, Clone)]
pub struct P2mApiService<S, P, C, M> {
    node_id: String,
    resource_map: Arc<Mutex<ResourceMap>>,
    flow_map: Arc<Mutex<FlowMap>>,
    sequencer: S,
    provenance: P,
    compliance: C,
    m2m: M,
}

impl<S, P, C, M> P2mApiService<S, P, C, M> {
    pub fn new(sequencer: S, provenance: P, compliance: C, m2m: M) -> Self {
        Self::new_with_node_id(String::new(), sequencer, provenance, compliance, m2m)
    }

    pub fn new_with_node_id(
        node_id: String,
        sequencer: S,
        provenance: P,
        compliance: C,
        m2m: M,
    ) -> Self {
        Self {
            node_id,
            resource_map: Arc::new(Mutex::new(HashMap::new())),
            flow_map: Arc::new(Mutex::new(HashMap::new())),
            sequencer,
            provenance,
            compliance,
            m2m,
        }
    }
}

impl<S, P, C, M> Service<P2mRequest> for P2mApiService<S, P, C, M>
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
    M: Service<M2mRequest, Response = M2mResponse, Error = TraceabilityError>
        + Clone
        + Send
        + 'static,
    M::Future: Send,
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
        let mut m2m = std::mem::replace(&mut self.m2m, this.m2m.clone());
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
                                // If destination is a stream, query remote policy via m2m
                                // else if query local destination policy
                                // else return error
                                let destination_policy = if destination.resource.is_stream() {
                                    if let M2mResponse::Compliance(policies) = m2m
                                        .call(M2mRequest::GetConsistentCompliance {
                                            source: source.clone(),
                                            destination: destination.clone(),
                                        })
                                        .await?
                                    {
                                        policies.iter().next().cloned().unwrap_or_default()
                                    } else {
                                        return Err(TraceabilityError::InternalTrace2eError);
                                    }
                                } else if let ComplianceResponse::Policies(policies) = compliance
                                    .call(ComplianceRequest::GetPolicies {
                                        ids: HashSet::from([destination.clone()]),
                                    })
                                    .await?
                                {
                                    policies.iter().next().cloned().unwrap_or_default()
                                } else {
                                    return Err(TraceabilityError::InternalTrace2eError);
                                };

                                // For the moment, this will only retrieve the source policies available locally
                                // TODO: Implement remote policies retrieval via m2m
                                let source_policies =
                                    if let ProvenanceResponse::Provenance { derived_from } =
                                        provenance
                                            .call(ProvenanceRequest::GetProvenance {
                                                id: source.clone(),
                                            })
                                            .await?
                                    {
                                        if let ComplianceResponse::Policies(policies) = compliance
                                            .call(ComplianceRequest::GetPolicies {
                                                ids: derived_from,
                                            })
                                            .await?
                                        {
                                            policies
                                        } else {
                                            return Err(TraceabilityError::InternalTrace2eError);
                                        }
                                    } else {
                                        return Err(TraceabilityError::InternalTrace2eError);
                                    };
                                if let ComplianceResponse::Grant = compliance
                                    .call(ComplianceRequest::CheckCompliance {
                                        source_policies,
                                        destination_policy,
                                    })
                                    .await?
                                {
                                    Ok(P2mResponse::Grant(flow_id))
                                } else {
                                    Err(TraceabilityError::DirectPolicyViolation)
                                }
                            }
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
    use tower::{Service, ServiceBuilder, filter::FilterLayer};

    use crate::{
        traceability::{
            layers::{
                compliance::ComplianceService, provenance::ProvenanceService,
                sequencer::SequencerService,
            },
            validation::ResourceValidator,
        },
        transport::nop::M2mNop,
    };

    use super::*;

    #[tokio::test]
    async fn unit_trace2e_service_request_response() {
        let mut p2m_service = P2mApiService::new(
            SequencerService::default(),
            ProvenanceService::default(),
            ComplianceService::default(),
            M2mNop::default(),
        );

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
        let mut p2m_service = ServiceBuilder::new()
            .layer(FilterLayer::new(ResourceValidator::default()))
            .service(P2mApiService::new(
                SequencerService::default(),
                ProvenanceService::default(),
                ComplianceService::default(),
                M2mNop::default(),
            ));

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
        let mut p2m_service = ServiceBuilder::new()
            .layer(FilterLayer::new(ResourceValidator::default()))
            .service(P2mApiService::new(
                SequencerService::default(),
                ProvenanceService::default(),
                ComplianceService::default(),
                M2mNop::default(),
            ));

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
        let mut p2m_service = ServiceBuilder::new()
            .layer(FilterLayer::new(ResourceValidator::default()))
            .service(P2mApiService::new(
                SequencerService::default(),
                ProvenanceService::default(),
                ComplianceService::default(),
                M2mNop::default(),
            ));

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
