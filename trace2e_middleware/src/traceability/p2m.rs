use crate::traceability::{
    api::{
        ComplianceRequest, ComplianceResponse, M2mRequest, M2mResponse, P2mRequest, P2mResponse,
        ProvenanceRequest, ProvenanceResponse, SequencerRequest, SequencerResponse,
    },
    error::TraceabilityError,
    naming::{NodeId, Resource},
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
#[cfg(feature = "trace2e_tracing")]
use tracing::{debug, info};

type ResourceMap = HashMap<(i32, i32), (Resource, Resource)>;
type FlowMap = HashMap<u128, (Resource, Resource, bool)>;

#[derive(Debug, Clone)]
pub struct P2mApiService<S, P, C, M> {
    resource_map: Arc<Mutex<ResourceMap>>,
    flow_map: Arc<Mutex<FlowMap>>,
    sequencer: S,
    provenance: P,
    compliance: C,
    m2m: M,
}

impl<S, P, C, M> P2mApiService<S, P, C, M> {
    pub fn new(sequencer: S, provenance: P, compliance: C, m2m: M) -> Self {
        Self {
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
        + NodeId
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
        let resource_map = self.resource_map.clone();
        let flow_map = self.flow_map.clone();
        let mut sequencer = self.sequencer.clone();
        let mut provenance = self.provenance.clone();
        let mut compliance = self.compliance.clone();
        let mut m2m = self.m2m.clone();
        Box::pin(async move {
            match request {
                P2mRequest::LocalEnroll { pid, fd, path } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[p2m-{}] LocalEnroll: pid: {}, fd: {}, path: {}",
                        provenance.node_id(),
                        pid,
                        fd,
                        path
                    );
                    resource_map.lock().await.insert(
                        (pid, fd),
                        (Resource::new_process(pid), Resource::new_file(path)),
                    );
                    Ok(P2mResponse::Ack)
                }
                P2mRequest::RemoteEnroll {
                    pid,
                    fd,
                    local_socket,
                    peer_socket,
                } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[p2m-{}] RemoteEnroll: pid: {}, fd: {}, local_socket: {}, peer_socket: {}",
                        provenance.node_id(),
                        pid,
                        fd,
                        local_socket,
                        peer_socket
                    );
                    resource_map.lock().await.insert(
                        (pid, fd),
                        (
                            Resource::new_process(pid),
                            Resource::new_stream(local_socket, peer_socket),
                        ),
                    );
                    Ok(P2mResponse::Ack)
                }
                P2mRequest::IoRequest { pid, fd, output } => {
                    if let Some((process, fd)) = resource_map.lock().await.get(&(pid, fd)) {
                        let flow_id = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)
                        {
                            Ok(n) => n.as_nanos(),
                            Err(_) => return Err(TraceabilityError::SystemTimeError),
                        };

                        let (source, destination) = if output {
                            (process.clone(), fd.clone())
                        } else {
                            (fd.clone(), process.clone())
                        };
                        #[cfg(feature = "trace2e_tracing")]
                        info!(
                            "[p2m-{}] IoRequest: source: {:?}, destination: {:?}",
                            provenance.node_id(),
                            source,
                            destination
                        );
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
                                let destination_policy = if let Some(remote_stream) =
                                    destination.is_stream()
                                {
                                    if let M2mResponse::Compliance(policies) = m2m
                                        .call(M2mRequest::GetConsistentCompliance {
                                            source: source.clone(),
                                            destination: remote_stream,
                                        })
                                        .await?
                                    {
                                        policies.iter().next().cloned().unwrap_or_default()
                                    } else {
                                        return Err(TraceabilityError::InternalTrace2eError);
                                    }
                                } else if let ComplianceResponse::Policies(policies) = compliance
                                    .call(ComplianceRequest::GetPolicies(HashSet::from([
                                        destination.clone(),
                                    ])))
                                    .await?
                                {
                                    policies.iter().next().cloned().unwrap_or_default()
                                } else {
                                    return Err(TraceabilityError::InternalTrace2eError);
                                };

                                let source_policies = match provenance
                                    .call(ProvenanceRequest::GetReferences(source))
                                    .await?
                                {
                                    ProvenanceResponse::Provenance {
                                        authority,
                                        mut references,
                                    } => {
                                        #[cfg(feature = "trace2e_tracing")]
                                        debug!(
                                            "[p2m-{}] Aggregated resources: {:?}",
                                            provenance.node_id(),
                                            references
                                        );
                                        // Get local source policies
                                        let mut source_policies = if let Some(local_references) =
                                            references.remove(&authority)
                                        {
                                            match compliance
                                                .call(ComplianceRequest::GetPolicies(
                                                    local_references,
                                                ))
                                                .await?
                                            {
                                                ComplianceResponse::Policies(policies) => {
                                                    HashMap::from([(authority, policies)])
                                                }
                                                _ => {
                                                    return Err(
                                                        TraceabilityError::InternalTrace2eError,
                                                    );
                                                }
                                            }
                                        } else {
                                            HashMap::new()
                                        };

                                        // Get remote source policies
                                        // Collect all futures without awaiting them
                                        let mut tasks = Vec::new();
                                        for (node_id, resources) in references {
                                            let mut m2m_clone = m2m.clone();
                                            let node_id_clone = node_id.clone();
                                            let task = tokio::spawn(async move {
                                                let result = m2m_clone
                                                    .call(M2mRequest::GetLooseCompliance {
                                                        authority_ip: node_id_clone.clone(),
                                                        resources,
                                                    })
                                                    .await;
                                                (node_id_clone, result)
                                            });
                                            tasks.push(task);
                                        }

                                        // Await all requests concurrently
                                        for task in tasks {
                                            let (node_id, result) = task.await.map_err(|_| {
                                                TraceabilityError::InternalTrace2eError
                                            })?;
                                            match result? {
                                                M2mResponse::Compliance(policies) => {
                                                    source_policies.insert(node_id, policies);
                                                }
                                                _ => {
                                                    return Err(
                                                        TraceabilityError::InternalTrace2eError,
                                                    );
                                                }
                                            }
                                        }

                                        source_policies
                                    }
                                    _ => return Err(TraceabilityError::InternalTrace2eError),
                                };
                                match compliance
                                    .call(ComplianceRequest::CheckCompliance {
                                        source_policies,
                                        destination_policy,
                                    })
                                    .await
                                {
                                    Ok(ComplianceResponse::Grant) => {
                                        flow_map
                                            .lock()
                                            .await
                                            .insert(flow_id, (process.clone(), fd.clone(), output));
                                        Ok(P2mResponse::Grant(flow_id))
                                    }
                                    Err(TraceabilityError::DirectPolicyViolation) => {
                                        // Release the flow if the policy is violated
                                        #[cfg(feature = "trace2e_tracing")]
                                        info!(
                                            "[p2m-{}] Release flow: {:?} as it is not compliant",
                                            provenance.node_id(),
                                            flow_id
                                        );
                                        sequencer
                                            .call(SequencerRequest::ReleaseFlow { destination })
                                            .await?;
                                        Err(TraceabilityError::DirectPolicyViolation)
                                    }
                                    _ => Err(TraceabilityError::InternalTrace2eError),
                                }
                            }
                            _ => Err(TraceabilityError::InternalTrace2eError),
                        }
                    } else {
                        Err(TraceabilityError::UndeclaredResource(pid, fd))
                    }
                }
                P2mRequest::IoReport { grant_id, .. } => {
                    let mut flow_map = flow_map.lock().await;
                    if let Some((process, fd, output)) = flow_map.remove(&grant_id) {
                        let (source, destination) =
                            if output { (process, fd) } else { (fd, process) };
                        #[cfg(feature = "trace2e_tracing")]
                        info!(
                            "[p2m-{}] IoReport: source: {:?}, destination: {:?}",
                            provenance.node_id(),
                            source,
                            destination
                        );
                        if let Some(remote_stream) = destination.is_stream() {
                            match provenance
                                .call(ProvenanceRequest::GetReferences(source))
                                .await?
                            {
                                ProvenanceResponse::Provenance { references, .. } => {
                                    m2m.call(M2mRequest::UpdateProvenance {
                                        source_prov: references,
                                        destination: remote_stream,
                                    })
                                    .await?;
                                }
                                _ => return Err(TraceabilityError::InternalTrace2eError),
                            }
                        } else {
                            provenance
                                .call(ProvenanceRequest::UpdateProvenance {
                                    source,
                                    destination: destination.clone(),
                                })
                                .await?;
                        }
                        sequencer
                            .call(SequencerRequest::ReleaseFlow { destination })
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
            core::{
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
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
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
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
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
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
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
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
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
