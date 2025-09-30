//! Process-to-Middleware (P2M) API service implementation.
//!
//! This module provides the core service implementation for handling requests from application
//! processes that need traceability tracking for their I/O operations. The P2M API is the
//! primary interface through which applications integrate with the trace2e system.
//!
//! ## Service Architecture
//!
//! The `P2mApiService` acts as the central coordinator between four key services:
//! - **Sequencer Service**: Manages flow ordering and resource reservations
//! - **Provenance Service**: Tracks data provenance  
//! - **Compliance Service**: Enforces policies and authorization decisions
//! - **M2M Client**: Communicates with remote middleware for distributed flows
//!
//! ## Resource Management
//!
//! The service maintains two primary data structures:
//! - **Resource Map**: Associates process/file descriptor pairs with source/destination resources
//! - **Flow Map**: Tracks active flows by grant ID for operation completion reporting
//!
//! ## Operation Workflow
//!
//! 1. **Enrollment**: Processes register their files and streams before use
//! 2. **Authorization**: Processes request permission for specific I/O operations
//! 3. **Execution**: Middleware evaluates policies and grants/denies access
//! 4. **Reporting**: Processes report completion status for audit trails
//!
//! ## Cross-Node Coordination
//!
//! For distributed flows involving remote resources, the service coordinates with
//! remote middleware instances via the M2M API to ensure consistent policy
//! enforcement and provenance tracking across the network.

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
    naming::{NodeId, Resource},
    validation::ResourceValidator,
};

/// Maps (process_id, file_descriptor) to (source_resource, destination_resource) pairs
type ResourceMap = DashMap<(i32, i32), (Resource, Resource)>;
/// Maps flow_id to (source_resource, destination_resource) pairs for active flows
type FlowMap = DashMap<u128, (Resource, Resource)>;

/// P2M (Process-to-Middleware) API Service.
///
/// Central orchestrator for process-initiated traceability operations. This service
/// manages the complete lifecycle of tracked I/O operations from initial resource
/// enrollment through final completion reporting.
///
/// ## Core Responsibilities
///
/// **Resource Enrollment**: Maintains registry of process file descriptors and their
/// associated resources (files or network streams) for traceability tracking.
///
/// **Flow Authorization**: Coordinates with compliance and sequencer services to
/// evaluate whether requested I/O operations should be permitted based on current policies.
///
/// **Distributed Coordination**: Communicates with remote middleware instances for
/// cross-node flows, ensuring consistent policy enforcement across the network.
///
/// **Provenance Tracking**: Updates provenance records following successful operations
/// to maintain complete audit trails for compliance and governance.
///
/// ## Concurrency and State Management
///
/// Uses concurrent data structures (`DashMap`) to handle multiple simultaneous requests
/// from different processes while maintaining consistency. Resource and flow maps are
/// shared across service instances using `Arc` for efficient cloning.
///
/// ## Generic Type Parameters
///
/// - `S`: Sequencer service for flow coordination and resource reservations
/// - `P`: Provenance service for provenance tracking
/// - `C`: Compliance service for policy evaluation and authorization decisions  
/// - `M`: M2M client service for communication with remote middleware instances
#[derive(Debug, Clone)]
pub struct P2mApiService<S, P, C, M> {
    /// Maps (process_id, file_descriptor) to (source_resource, destination_resource) pairs
    resource_map: Arc<ResourceMap>,
    /// Maps flow_id to (source_resource, destination_resource) pairs for active flows
    flow_map: Arc<FlowMap>,
    /// Service for managing flows sequencing
    sequencer: S,
    /// Service for tracking resources provenance
    provenance: P,
    /// Service for policy management and compliance checking
    compliance: C,
    /// Client service for Middleware-to-Middleware communication
    m2m: M,
    /// Whether to perform resource validation on incoming requests
    enable_resource_validation: bool,
}

impl<S, P, C, M> P2mApiService<S, P, C, M> {
    /// Creates a new P2M API service with the provided component services.
    ///
    /// Initializes empty resource and flow maps and stores references to the
    /// core services needed for traceability operations. The service is ready
    /// to handle process requests immediately after construction.
    ///
    /// # Arguments
    /// * `sequencer` - Service for flow coordination and resource reservations
    /// * `provenance` - Service for provenance tracking
    /// * `compliance` - Service for policy evaluation and authorization decisions
    /// * `m2m` - Client for communication with remote middleware instances
    pub fn new(sequencer: S, provenance: P, compliance: C, m2m: M) -> Self {
        Self {
            resource_map: Arc::new(ResourceMap::new()),
            flow_map: Arc::new(FlowMap::new()),
            sequencer,
            provenance,
            compliance,
            m2m,
            enable_resource_validation: false,
        }
    }

    /// Pre-enrolls resources for testing and simulation purposes.
    ///
    /// Creates mock enrollments for the specified number of processes, files, and streams
    /// to support testing scenarios without requiring actual process interactions.
    /// Should only be used in test environments or for system benchmarking.
    ///
    /// # Arguments
    /// * `process_count` - Number of mock processes to enroll
    /// * `per_process_file_count` - Number of files to enroll per process
    /// * `per_process_stream_count` - Number of streams to enroll per process
    ///
    /// # Returns
    /// The service instance with pre-enrolled mock resources
    #[cfg(test)]
    pub fn with_enrolled_resources(
        self,
        process_count: u32,
        per_process_file_count: u32,
        per_process_stream_count: u32,
    ) -> Self {
        // Pre-calculate all entries to avoid repeated allocations during insertion
        let file_entries: Vec<_> = (0..process_count as i32)
            .flat_map(|process_id| {
                (3..(per_process_file_count + 3) as i32).map(move |file_id| {
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
                ((per_process_file_count + 3) as i32
                    ..(per_process_stream_count + per_process_file_count + 3) as i32)
                    .map(move |stream_id| {
                        (
                            (process_id, stream_id),
                            (
                                Resource::new_process_mock(process_id),
                                Resource::new_stream(
                                    format!("127.0.0.1:{stream_id}",),
                                    format!("127.0.0.2:{stream_id}",),
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

    /// Enables or disables resource validation for incoming P2M requests.
    ///
    /// When validation is enabled, all incoming requests are validated for:
    /// - Valid process IDs (must correspond to running processes)
    /// - Valid stream addresses (must be well-formed and compatible)
    ///
    /// This method uses the same ResourceValidator logic as the Tower filter
    /// but integrates it directly into the service to avoid complex Send/Sync
    /// constraints with async runtimes.
    ///
    /// # Arguments
    /// * `enable` - Whether to enable resource validation
    ///
    /// # Returns
    /// Self with validation setting applied
    pub fn with_resource_validation(mut self, enable: bool) -> Self {
        self.enable_resource_validation = enable;
        self
    }

    /// Validates a P2M request according to resource requirements.
    ///
    /// Applies the same validation rules as the ResourceValidator:
    /// - `RemoteEnroll`: Validates both process and stream resources
    /// - `LocalEnroll`, `IoRequest`: Validates process resources only  
    /// - `IoReport`: Passes through without validation (grant ID is validated later)
    ///
    /// # Arguments
    /// * `request` - The P2M request to validate
    ///
    /// # Returns
    /// `Ok(())` if validation passes, `Err(TraceabilityError)` if validation fails
    ///
    /// # Errors
    /// - `InvalidProcess`: When the process ID is not found or accessible
    /// - `InvalidStream`: When socket addresses are malformed or incompatible
    fn validate_request(request: &P2mRequest) -> Result<&P2mRequest, TraceabilityError> {
        // Use the same validation logic as the ResourceValidator
        match request {
            P2mRequest::RemoteEnroll { pid, local_socket, peer_socket, .. } => {
                if ResourceValidator.is_valid_process(*pid) {
                    if ResourceValidator.is_valid_stream(local_socket, peer_socket) {
                        Ok(request)
                    } else {
                        Err(TraceabilityError::InvalidStream(
                            local_socket.clone(),
                            peer_socket.clone(),
                        ))
                    }
                } else {
                    Err(TraceabilityError::InvalidProcess(*pid))
                }
            }
            P2mRequest::LocalEnroll { pid, .. } | P2mRequest::IoRequest { pid, .. } => {
                if ResourceValidator.is_valid_process(*pid) {
                    Ok(request)
                } else {
                    Err(TraceabilityError::InvalidProcess(*pid))
                }
            }
            P2mRequest::IoReport { .. } => Ok(request),
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
        let enable_validation = self.enable_resource_validation;

        Box::pin(async move {
            // Perform resource validation if enabled
            if enable_validation {
                Self::validate_request(&request)?;
            }

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
                                // If destination is a stream, query remote policy via m2m
                                // else if query local destination policy
                                // else return error

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
                                        let mut source_policies = if let Some(local_references) =
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
                                            let (node_id, result) = task.await.map_err(|_| {
                                                TraceabilityError::InternalTrace2eError
                                            })?;
                                            match result? {
                                                M2mResponse::SourceCompliance(policies) => {
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
                                    _ => {
                                        return Err(TraceabilityError::InternalTrace2eError);
                                    }
                                };
                                match compliance
                                    .call(ComplianceRequest::EvalPolicies {
                                        source_policies,
                                        destination: destination.clone(),
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
                    if let Some((_, (source, destination))) = flow_map.remove(&grant_id) {
                        #[cfg(feature = "trace2e_tracing")]
                        info!(
                            "[p2m-{}] IoReport: source: {:?}, destination: {:?}",
                            provenance.node_id(),
                            source,
                            destination
                        );
                        if let Some(remote_stream) = destination.is_stream() {
                            match provenance
                                .call(ProvenanceRequest::GetReferences(source.clone()))
                                .await?
                            {
                                ProvenanceResponse::Provenance(references) => {
                                    m2m.ready()
                                        .await?
                                        .call(M2mRequest::UpdateProvenance {
                                            source_prov: references,
                                            destination: remote_stream,
                                        })
                                        .await?;
                                }
                                _ => return Err(TraceabilityError::InternalTrace2eError),
                            };
                        }

                        provenance
                            .call(ProvenanceRequest::UpdateProvenance {
                                source,
                                destination: destination.clone(),
                            })
                            .await?;

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
    use tower::Service;

    use super::*;
    use crate::{
        traceability::core::{
            compliance::ComplianceService, provenance::ProvenanceService,
            sequencer::SequencerService,
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
        let mut p2m_service = P2mApiService::new(
            SequencerService::default(),
            ProvenanceService::default(),
            ComplianceService::default(),
            M2mNop,
        )
        .with_resource_validation(true);

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
        let mut p2m_service = P2mApiService::new(
            SequencerService::default(),
            ProvenanceService::default(),
            ComplianceService::default(),
            M2mNop,
        )
        .with_resource_validation(true);

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
        let mut p2m_service = P2mApiService::new(
            SequencerService::default(),
            ProvenanceService::default(),
            ComplianceService::default(),
            M2mNop,
        )
        .with_resource_validation(true);

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

    #[tokio::test]
    async fn unit_trace2e_service_integrated_validation() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();

        // Test P2M service with integrated validation enabled
        let mut p2m_service_with_validation = P2mApiService::new(
            SequencerService::default(),
            ProvenanceService::default(),
            ComplianceService::default(),
            M2mNop,
        )
        .with_resource_validation(true);

        // Test P2M service with validation disabled
        let mut p2m_service_without_validation = P2mApiService::new(
            SequencerService::default(),
            ProvenanceService::default(),
            ComplianceService::default(),
            M2mNop,
        )
        .with_resource_validation(false);

        // Test invalid process - should fail with validation enabled
        assert_eq!(
            p2m_service_with_validation
                .call(P2mRequest::LocalEnroll { pid: 0, fd: 3, path: "/tmp/test.txt".to_string() })
                .await
                .unwrap_err()
                .to_string(),
            "Traceability error, process not found (pid: 0)"
        );

        // Test invalid process - should succeed with validation disabled
        assert_eq!(
            p2m_service_without_validation
                .call(P2mRequest::LocalEnroll { pid: 0, fd: 3, path: "/tmp/test.txt".to_string() })
                .await
                .unwrap(),
            P2mResponse::Ack
        );

        // Test valid process - should succeed with validation enabled
        assert_eq!(
            p2m_service_with_validation
                .call(P2mRequest::LocalEnroll {
                    pid: std::process::id() as i32,
                    fd: 3,
                    path: "/tmp/test.txt".to_string()
                })
                .await
                .unwrap(),
            P2mResponse::Ack
        );

        // Test valid process - should succeed with validation disabled
        assert_eq!(
            p2m_service_without_validation
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
}
