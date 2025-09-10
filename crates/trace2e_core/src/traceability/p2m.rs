use std::{
    collections::HashMap, future::Future, pin::Pin, sync::Arc, task::Poll, time::SystemTime,
};

use dashmap::DashMap;
use tower::{Service, ServiceExt};
#[cfg(feature = "trace2e_tracing")]
use tracing::{debug, info};

use crate::traceability::{
    api::{
        ComplianceRequest, ComplianceResponse, M2mRequest, M2mResponse, P2mRequest, P2mResponse,
        ProvenanceRequest, ProvenanceResponse, SequencerRequest, SequencerResponse,
    },
    error::TraceabilityError,
    mode::Mode,
    naming::{NodeId, Resource},
};

type ResourceMap = DashMap<(i32, i32), (Resource, Resource)>;
type FlowMap = DashMap<u128, (Resource, Resource)>;

/// P2M (Process-to-Middleware) API Service
///
/// This service handles traceability requests from processes, managing resource enrollment,
/// I/O flow authorization, and provenance tracking. It coordinates between sequencer,
/// provenance, compliance, and Middleware-to-Middleware communication services.
#[derive(Debug, Clone)]
pub struct P2mApiService<S, P, C, M> {
    /// Maps (process_id, file_descriptor) to (source_resource, destination_resource) pairs
    resource_map: Arc<ResourceMap>,
    /// Maps flow_id to (source_resource, destination_resource) pairs for active flows
    flow_map: Arc<FlowMap>,
    /// Compliance propagation mode
    mode: Mode,
    /// Service for managing flows sequencing
    sequencer: S,
    /// Service for tracking resources provenance
    provenance: P,
    /// Service for policy management and compliance checking
    compliance: C,
    /// Client service for Middleware-to-Middleware communication
    m2m: M,
}

impl<S, P, C, M> P2mApiService<S, P, C, M> {
    /// Creates a new P2M API service with the provided component services
    pub fn new(sequencer: S, provenance: P, compliance: C, m2m: M) -> Self {
        Self {
            resource_map: Arc::new(ResourceMap::new()),
            flow_map: Arc::new(FlowMap::new()),
            mode: Default::default(),
            sequencer,
            provenance,
            compliance,
            m2m,
        }
    }

    /// Sets the compliance propagation mode
    pub fn with_mode(self, mode: Mode) -> Self {
        Self { mode, ..self }
    }

    /// Enrolls the given number of processes and files per process for testing/mocking purposes.
    pub fn with_enrolled_resources(
        self,
        process_count: u32,
        per_process_file_count: u32,
        per_process_stream_count: u32,
    ) -> Self {
        // Pre-calculate all entries to avoid repeated allocations during insertion
        let file_entries: Vec<_> = (0..process_count as i32)
            .flat_map(|process_id| {
                (0..per_process_file_count as i32).map(move |file_id| {
                    (
                        (process_id, file_id),
                        (
                            Resource::new_process_mock(process_id),
                            Resource::new_file(format!(
                                "/file_{}",
                                (process_id + file_id) % process_count as i32
                            )),
                        ),
                    )
                })
            })
            .collect();
        let stream_entries: Vec<_> = (0..process_count as i32)
            .flat_map(|process_id| {
                (0..per_process_stream_count as i32).map(move |stream_id| {
                    (
                        (process_id, stream_id),
                        (
                            Resource::new_process_mock(process_id),
                            Resource::new_stream(
                                format!("127.0.0.1:{}", stream_id),
                                format!("127.0.0.2:{}", stream_id),
                            ),
                        ),
                    )
                })
            })
            .collect();

        // Batch insert all entries at once using DashMap's concurrent insert capabilities
        for (key, value) in file_entries.into_iter().chain(stream_entries.into_iter()) {
            self.resource_map.insert(key, value);
        }
        self
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
        let mode = self.mode;
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
                    resource_map
                        .insert((pid, fd), (Resource::new_process(pid), Resource::new_file(path)));
                    Ok(P2mResponse::Ack)
                }
                P2mRequest::RemoteEnroll { pid, fd, local_socket, peer_socket } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[p2m-{}] RemoteEnroll: pid: {}, fd: {}, local_socket: {}, peer_socket: {}",
                        provenance.node_id(),
                        pid,
                        fd,
                        local_socket,
                        peer_socket
                    );
                    resource_map.insert(
                        (pid, fd),
                        (
                            Resource::new_process(pid),
                            Resource::new_stream(local_socket, peer_socket),
                        ),
                    );
                    Ok(P2mResponse::Ack)
                }
                P2mRequest::IoRequest { pid, fd, output } => {
                    if let Some(resource) = resource_map.get(&(pid, fd)) {
                        let flow_id = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)
                        {
                            Ok(n) => n.as_nanos(),
                            Err(_) => return Err(TraceabilityError::SystemTimeError),
                        };
                        let (source, destination) = if output {
                            (resource.0.to_owned(), resource.1.to_owned())
                        } else {
                            (resource.1.to_owned(), resource.0.to_owned())
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
                                match mode {
                                    Mode::Pull => {
                                        // If destination is a stream, query remote policy via m2m
                                        // else if query local destination policy
                                        // else return error
                                        let destination_policy = if let Some(remote_stream) =
                                            destination.is_stream()
                                            && mode == Mode::Pull
                                        {
                                            if let M2mResponse::DestinationCompliance(policy) = m2m
                                                .ready()
                                                .await?
                                                .call(M2mRequest::GetDestinationCompliance {
                                                    source: source.clone(),
                                                    destination: remote_stream,
                                                })
                                                .await?
                                            {
                                                policy
                                            } else {
                                                return Err(
                                                    TraceabilityError::InternalTrace2eError,
                                                );
                                            }
                                        } else if let ComplianceResponse::Policy(policy) =
                                            compliance
                                                .call(ComplianceRequest::GetPolicy(
                                                    destination.clone(),
                                                ))
                                                .await?
                                        {
                                            policy
                                        } else {
                                            return Err(TraceabilityError::InternalTrace2eError);
                                        };

                                        let source_policies = match provenance
                                            .call(ProvenanceRequest::GetReferences(source.clone()))
                                            .await?
                                        {
                                            ProvenanceResponse::Provenance(mut references) => {
                                                #[cfg(feature = "trace2e_tracing")]
                                                debug!(
                                                    "[p2m-{}] Aggregated resources: {:?}",
                                                    provenance.node_id(),
                                                    references
                                                );
                                                // Get local source policies
                                                let mut source_policies = if let Some(
                                                    local_references,
                                                ) =
                                                    references.remove(&provenance.node_id())
                                                {
                                                    match compliance
                                                        .call(ComplianceRequest::GetPolicies(
                                                            local_references,
                                                        ))
                                                        .await?
                                                    {
                                                        ComplianceResponse::Policies(policies) => {
                                                            HashMap::from([(
                                                                provenance.node_id(),
                                                                policies,
                                                            )])
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
                                                        let result =
                                                            match m2m_clone.ready().await {
                                                                Ok(ready_service) => ready_service
                                                                    .call(M2mRequest::GetSourceCompliance {
                                                                        authority_ip: node_id_clone.clone(),
                                                                        resources,
                                                                    })
                                                                    .await,
                                                                Err(e) => Err(e),
                                                            };
                                                        (node_id_clone, result)
                                                    });
                                                    tasks.push(task);
                                                }

                                                // Await all requests concurrently
                                                for task in tasks {
                                                    let (node_id, result) =
                                                        task.await.map_err(|_| {
                                                            TraceabilityError::InternalTrace2eError
                                                        })?;
                                                    match result? {
                                                        M2mResponse::SourceCompliance(policies) => {
                                                            source_policies
                                                                .insert(node_id, policies);
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
                                            _ => {
                                                return Err(
                                                    TraceabilityError::InternalTrace2eError,
                                                );
                                            }
                                        };
                                        match compliance
                                            .call(ComplianceRequest::EvalPolicies {
                                                source_policies,
                                                destination_policy,
                                            })
                                            .await
                                        {
                                            Ok(ComplianceResponse::Grant) => {
                                                flow_map.insert(flow_id, (source, destination));
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
                                                    .call(SequencerRequest::ReleaseFlow {
                                                        destination,
                                                    })
                                                    .await?;
                                                Err(TraceabilityError::DirectPolicyViolation)
                                            }
                                            _ => Err(TraceabilityError::InternalTrace2eError),
                                        }
                                    }
                                    Mode::Push => {
                                        match provenance
                                            .call(ProvenanceRequest::GetReferences(source.clone()))
                                            .await?
                                        {
                                            ProvenanceResponse::Provenance(sources) => {
                                                match compliance
                                                    .call(ComplianceRequest::CheckCompliance {
                                                        sources,
                                                        destination: destination.clone(),
                                                    })
                                                    .await
                                                {
                                                    Ok(ComplianceResponse::Grant) => {
                                                        flow_map
                                                            .insert(flow_id, (source, destination));
                                                        Ok(P2mResponse::Grant(flow_id))
                                                    }
                                                    Err(
                                                        TraceabilityError::DirectPolicyViolation,
                                                    ) => {
                                                        // Release the flow if the policy is violated
                                                        #[cfg(feature = "trace2e_tracing")]
                                                        info!(
                                                            "[p2m-{}] Release flow: {:?} as it is not compliant",
                                                            provenance.node_id(),
                                                            flow_id
                                                        );
                                                        sequencer
                                                            .call(SequencerRequest::ReleaseFlow {
                                                                destination,
                                                            })
                                                            .await?;
                                                        Err(TraceabilityError::DirectPolicyViolation)
                                                    }
                                                    _ => {
                                                        Err(TraceabilityError::InternalTrace2eError)
                                                    }
                                                }
                                            }
                                            _ => Err(TraceabilityError::InternalTrace2eError),
                                        }
                                    }
                                }
                            }
                            _ => Err(TraceabilityError::InternalTrace2eError),
                        }
                    } else {
                        Err(TraceabilityError::UndeclaredResource(pid, fd))
                    }
                }
                P2mRequest::IoReport { grant_id, .. } => {
                    if let Some((_, (source, destination))) = flow_map.remove(&grant_id) {
                        #[cfg(feature = "trace2e_tracing")]
                        info!(
                            "[p2m-{}] IoReport: source: {:?}, destination: {:?}",
                            provenance.node_id(),
                            source,
                            destination
                        );
                        if let Some(remote_stream) = destination.is_stream() {
                            let references = match provenance
                                .call(ProvenanceRequest::GetReferences(source))
                                .await?
                            {
                                ProvenanceResponse::Provenance(references) => {
                                    m2m.ready()
                                        .await?
                                        .call(M2mRequest::UpdateProvenance {
                                            source_prov: references.clone(),
                                            destination: remote_stream.clone(),
                                        })
                                        .await?;
                                    references
                                }
                                _ => return Err(TraceabilityError::InternalTrace2eError),
                            };
                            match compliance
                                .call(ComplianceRequest::GetPoliciesBatch(references))
                                .await?
                            {
                                ComplianceResponse::PoliciesBatch(policies) => {
                                    m2m.ready()
                                        .await?
                                        .call(M2mRequest::UpdatePolicies {
                                            policies,
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
                        sequencer.call(SequencerRequest::ReleaseFlow { destination }).await?;
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

    use super::*;
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

    #[tokio::test]
    async fn unit_trace2e_service_request_response() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut p2m_service = P2mApiService::new(
            SequencerService::default(),
            ProvenanceService::default(),
            ComplianceService::default(),
            M2mNop,
        );

        assert_eq!(
            p2m_service
                .call(P2mRequest::LocalEnroll { pid: 1, fd: 3, path: "/tmp/test.txt".to_string() })
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

        let P2mResponse::Grant(flow_id) =
            p2m_service.call(P2mRequest::IoRequest { pid: 1, fd: 3, output: true }).await.unwrap()
        else {
            panic!("Expected P2mResponse::Grant");
        };
        assert_eq!(
            p2m_service
                .call(P2mRequest::IoReport { pid: 1, fd: 3, grant_id: flow_id, result: true })
                .await
                .unwrap(),
            P2mResponse::Ack
        );

        let P2mResponse::Grant(flow_id) =
            p2m_service.call(P2mRequest::IoRequest { pid: 1, fd: 3, output: false }).await.unwrap()
        else {
            panic!("Expected P2mResponse::Grant");
        };
        assert_eq!(
            p2m_service
                .call(P2mRequest::IoReport { pid: 1, fd: 3, grant_id: flow_id, result: true })
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
            .layer(FilterLayer::new(ResourceValidator))
            .service(P2mApiService::new(
                SequencerService::default(),
                ProvenanceService::default(),
                ComplianceService::default(),
                M2mNop,
            ));

        // Test with invalid process
        // This request is supposed to be filtered out by the validator
        assert_eq!(
            p2m_service
                .call(P2mRequest::LocalEnroll { pid: 0, fd: 3, path: "/tmp/test.txt".to_string() })
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
            .layer(FilterLayer::new(ResourceValidator))
            .service(P2mApiService::new(
                SequencerService::default(),
                ProvenanceService::default(),
                ComplianceService::default(),
                M2mNop,
            ));

        // Neither process nor fd are enrolled
        assert_eq!(
            p2m_service
                .call(P2mRequest::IoRequest { pid: std::process::id() as i32, fd: 3, output: true })
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
                .call(P2mRequest::IoRequest { pid: std::process::id() as i32, fd: 3, output: true })
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
            .layer(FilterLayer::new(ResourceValidator))
            .service(P2mApiService::new(
                SequencerService::default(),
                ProvenanceService::default(),
                ComplianceService::default(),
                M2mNop,
            ));

        // Invalid grant id
        assert_eq!(
            p2m_service
                .call(P2mRequest::IoReport { pid: 1, fd: 3, grant_id: 0, result: true })
                .await
                .unwrap_err()
                .to_string(),
            "Traceability error, flow not found (id: 0)"
        );
    }
}
