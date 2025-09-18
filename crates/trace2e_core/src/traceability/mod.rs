//! Traceability module.
//!
//! This module provides a complete traceability solution for distributed systems,
//! enabling comprehensive data provenance tracking, policy enforcement, and compliance
//! monitoring across process boundaries and network connections.
//!
//! ## Core Architecture
//!
//! The traceability system is built around three main API layers:
//!
//! ### Process-to-Middleware (P2M) API
//! Primary interface for application processes to register resources and request
//! I/O operations with traceability guarantees. Handles resource enrollment,
//! authorization requests, and completion reporting.
//!
//! ### Middleware-to-Middleware (M2M) API  
//! Enables communication between distributed middleware instances for cross-node
//! policy evaluation, flow coordination, and provenance synchronization.
//!
//! ### Operator-to-Middleware (O2M) API
//! Administrative interface for policy management, compliance configuration,
//! and provenance analysis by external operators and organizations.
//!
//! ## Service Components
//!
//! ### Core Services
//! - **Sequencer**: Manages flow ordering and resource reservations to prevent race conditions
//! - **Provenance**: Tracks data provenance across operations
//! - **Compliance**: Enforces organizational policies and regulatory requirements
//!
//! ### Infrastructure Services
//! - **Validation**: Validates incoming requests and resource accessibility
//! - **Naming**: Provides unified resource identification and naming conventions
//! - **Error Handling**: Comprehensive error types and handling for operational monitoring
//!
//! ## Default Service Stacks
//!
//! Pre-configured service combinations are available as type aliases for common deployment patterns:
//! - `M2mApiDefaultStack`: Standard M2M service with default component configuration
//! - `P2mApiDefaultStack<M>`: Standard P2M service parameterized by M2M client type
//! - `O2mApiDefaultStack`: Standard O2M service with default component configuration
//!
//! ## Initialization Helpers
//!
//! The module provides helper functions for initializing complete middleware stacks:
//! - `init_middleware()`: Initialize production middleware with specified M2M client
//! - `init_middleware_with_enrolled_resources()`: Initialize with pre-enrolled test resources
pub mod api;
pub mod core;
pub mod error;
pub mod m2m;
pub mod naming;
pub mod o2m;
pub mod p2m;
pub mod validation;

/// Standard M2M API service stack with default component configuration.
///
/// Combines sequencer, provenance, and compliance services using the default
/// waiting queue mechanism for flow coordination. Suitable for most production
/// deployments requiring standard M2M functionality.
pub type M2mApiDefaultStack = m2m::M2mApiService<
    core::sequencer::WaitingQueueService<core::sequencer::SequencerService>,
    core::provenance::ProvenanceService,
    core::compliance::ComplianceService,
>;

/// Standard P2M API service stack parameterized by M2M client type.
///
/// Combines sequencer, provenance, and compliance services with a configurable
/// M2M client for distributed coordination. The generic parameter `M` allows
/// different M2M transport implementations to be used.
pub type P2mApiDefaultStack<M> = p2m::P2mApiService<
    core::sequencer::WaitingQueueService<core::sequencer::SequencerService>,
    core::provenance::ProvenanceService,
    core::compliance::ComplianceService,
    M,
>;

/// Standard O2M API service stack with default component configuration.
///
/// Combines provenance and compliance services for administrative operations.
/// Does not include sequencer as O2M operations are typically read-heavy
/// and don't require flow coordination.
pub type O2mApiDefaultStack =
    o2m::O2mApiService<core::provenance::ProvenanceService, core::compliance::ComplianceService>;

/// Initialize a complete middleware stack for production deployment.
///
/// Creates a fully configured middleware stack with all three API services
/// (M2M, P2M, O2M) using default component configurations. This is the
/// standard initialization method for production deployments.
///
/// # Arguments
/// * `node_id` - Unique identifier for this middleware node in the distributed system
/// * `max_retries` - Maximum retry attempts for the waiting queue (None for unlimited)
/// * `m2m_client` - Client service for M2M communication with remote middleware
/// * `enable_resource_validation` - Whether to enable resource validation for P2M requests
///
/// # Returns
/// A tuple containing (M2M service, P2M service, O2M service) ready for use
///
/// # Type Parameters
/// * `M` - M2M client type that implements the required service traits
pub fn init_middleware<M>(
    node_id: String,
    max_retries: Option<u32>,
    m2m_client: M,
    enable_resource_validation: bool,
) -> (M2mApiDefaultStack, P2mApiDefaultStack<M>, O2mApiDefaultStack)
where
    M: tower::Service<
            api::M2mRequest,
            Response = api::M2mResponse,
            Error = error::TraceabilityError,
        > + Clone
        + Send
        + 'static,
    M::Future: Send,
{
    init_middleware_with_enrolled_resources(node_id, max_retries, m2m_client, enable_resource_validation, 0, 0, 0)
}

/// Initialize a middleware stack with pre-enrolled resources for testing.
///
/// Creates a middleware stack identical to `init_middleware()` but with
/// pre-enrolled mock resources. This is useful for testing, benchmarking,
/// and simulation scenarios where actual process interactions are not needed.
///
/// # Arguments
/// * `node_id` - Unique identifier for this middleware node
/// * `max_retries` - Maximum retry attempts for the waiting queue
/// * `m2m_client` - Client service for M2M communication
/// * `enable_resource_validation` - Whether to enable resource validation for P2M requests
/// * `process_count` - Number of mock processes to pre-enroll
/// * `per_process_file_count` - Number of files to enroll per process
/// * `per_process_stream_count` - Number of streams to enroll per process
///
/// # Returns
/// A tuple containing (M2M service, P2M service, O2M service) with pre-enrolled resources
///
/// # Warning
/// Should only be used for testing purposes. Production deployments should use
/// `init_middleware()` and rely on actual process enrollment.
pub fn init_middleware_with_enrolled_resources<M>(
    node_id: String,
    max_retries: Option<u32>,
    m2m_client: M,
    enable_resource_validation: bool,
    _process_count: u32,
    _per_process_file_count: u32,
    _per_process_stream_count: u32,
) -> (M2mApiDefaultStack, P2mApiDefaultStack<M>, O2mApiDefaultStack)
where
    M: tower::Service<
            api::M2mRequest,
            Response = api::M2mResponse,
            Error = error::TraceabilityError,
        > + Clone
        + Send
        + 'static,
    M::Future: Send,
{
    let sequencer = tower::ServiceBuilder::new()
        .layer(tower::layer::layer_fn(|inner| {
            core::sequencer::WaitingQueueService::new(inner, max_retries)
        }))
        .service(core::sequencer::SequencerService::default());
    let provenance = core::provenance::ProvenanceService::new(node_id);
    let compliance = core::compliance::ComplianceService::default();

    let m2m_service: M2mApiDefaultStack =
        m2m::M2mApiService::new(sequencer.clone(), provenance.clone(), compliance.clone());
    
    #[cfg(test)]
    let p2m_service: P2mApiDefaultStack<M> =
        p2m::P2mApiService::new(sequencer, provenance.clone(), compliance.clone(), m2m_client)
            .with_resource_validation(enable_resource_validation)
            .with_enrolled_resources(
                _process_count,
                _per_process_file_count,
                _per_process_stream_count,
            );
    #[cfg(not(test))]
    let p2m_service: P2mApiDefaultStack<M> =
        p2m::P2mApiService::new(sequencer, provenance.clone(), compliance.clone(), m2m_client)
            .with_resource_validation(enable_resource_validation);

    let o2m_service: O2mApiDefaultStack = o2m::O2mApiService::new(provenance, compliance);

    (m2m_service, p2m_service, o2m_service)
}
