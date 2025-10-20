//! Traceability API type definitions.
//!
//! This module defines all request and response types for the trace2e traceability system,
//! which provides comprehensive data lineage tracking and compliance enforcement across
//! distributed systems.
//!
//! The API is organized into three main communication patterns:
//!
//! ## Process-to-Middleware (P2M) API
//! Enables processes to register resources (files, network streams) and request I/O operations
//! with traceability guarantees. Processes must first enroll their resources before performing
//! tracked I/O operations.
//!
//! ## Middleware-to-Middleware (M2M) API  
//! Facilitates communication between distributed middleware instances for cross-node compliance
//! checking, flow coordination, and provenance synchronization.
//!
//! ## Operator-to-Middleware (O2M) API
//! Provides administrative interfaces for policy management, compliance configuration, and
//! provenance querying by external operators and organizations.
//!
//! ## Internal Service APIs
//! Defines request/response types for internal services:
//! - **Sequencer**: Resource management and flow control
//! - **Provenance**: Data lineage tracking and ancestry management  
//! - **Compliance**: Policy enforcement and authorization decisions
//! - **Consent**: User consent management for data flows

use std::collections::{HashMap, HashSet};

use crate::traceability::{
    infrastructure::naming::Resource,
    services::{
        compliance::{ConfidentialityPolicy, Policy},
        consent::Destination,
    },
};

/// Process-to-Middleware (P2M) request types.
///
/// These requests are initiated by application processes to the middleware for resource
/// enrollment and I/O operation authorization. The workflow typically follows:
/// 1. Enroll resources using `LocalEnroll` or `RemoteEnroll`
/// 2. Request I/O permission using `IoRequest`  
/// 3. Report operation completion using `IoReport`
#[derive(Debug, Clone)]
pub enum P2mRequest {
    /// Register a file resource with the middleware for traceability tracking.
    ///
    /// Must be called before any I/O operations on the file. The middleware will
    /// create appropriate resource identifiers and establish compliance policies.
    LocalEnroll {
        /// Process identifier that opened the file
        pid: i32,
        /// File descriptor assigned by the operating system
        fd: i32,
        /// Absolute or relative path to the file
        path: String,
    },

    /// Register a network stream resource with the middleware for traceability tracking.
    ///
    /// Used for TCP connections.
    /// Both endpoints must be specified to enable proper flow tracking.
    RemoteEnroll {
        /// Process identifier that opened the stream
        pid: i32,
        /// File descriptor assigned by the operating system  
        fd: i32,
        /// Local socket address (e.g., "127.0.0.1:8080")
        local_socket: String,
        /// Remote peer socket address (e.g., "192.168.1.100:9000")
        peer_socket: String,
    },

    /// Request authorization to perform an I/O operation on a previously enrolled resource.
    ///
    /// The middleware will evaluate compliance policies, check data lineage requirements,
    /// and coordinate with remote middleware instances if necessary before granting access.
    IoRequest {
        /// Process identifier requesting the operation
        pid: i32,
        /// File descriptor for the target resource
        fd: i32,
        /// Direction of data flow: true for output (write), false for input (read)
        output: bool,
    },

    /// Report the completion status of a previously authorized I/O operation.
    ///
    /// This enables the middleware to update provenance records, release flow reservations,
    /// and maintain accurate audit trails for compliance purposes.
    IoReport {
        /// Process identifier that performed the operation
        pid: i32,
        /// File descriptor for the resource
        fd: i32,
        /// Grant identifier returned from the corresponding `IoRequest`
        grant_id: u128,
        /// Operation outcome: true for success, false for failure
        result: bool,
    },
}

/// Process-to-Middleware (P2M) response types.
///
/// Responses sent from the middleware back to application processes following P2M requests.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum P2mResponse {
    /// Authorization granted for an I/O operation with a unique grant identifier.
    ///
    /// Returned in response to `P2mRequest::IoRequest` when the operation is permitted
    /// by current policies. The grant ID must be included in the subsequent `IoReport`.
    Grant(u128),

    /// Acknowledgment of successful request processing.
    ///
    /// Returned for `LocalEnroll`, `RemoteEnroll`, and `IoReport` requests to confirm
    /// the operation was completed successfully.
    Ack,
}

/// Middleware-to-Middleware (M2M) request types.
///
/// These requests enable communication between distributed middleware instances to maintain
/// consistent compliance policies and provenance records across the network. Used primarily
/// for cross-node data flows and distributed policy enforcement.
#[derive(Debug, Clone)]
pub enum M2mRequest {
    /// Request compliance policies for a destination resource from its authoritative middleware.
    ///
    /// Used when a local middleware needs to evaluate whether a data flow to a remote
    /// resource is permitted. The source middleware queries the destination middleware
    /// to obtain current policies before authorizing the operation.
    GetDestinationCompliance {
        /// Source resource where data originates
        source: Resource,
        /// Destination resource where data will be written
        destination: Resource,
    },

    /// Request compliance policies for source resources from their authoritative middleware.
    ///
    /// Used by destination middleware to verify that incoming data flows comply with
    /// source policies and organizational requirements.
    GetSourceCompliance {
        /// IP address of the authoritative middleware for the source resources
        authority_ip: String,
        /// Set of source resources to query policies for
        resources: HashSet<Resource>,
    },

    /// Update provenance records on the destination middleware following a cross-node data flow.
    ///
    /// Transfers lineage information from source to destination to maintain complete
    /// provenance chains across distributed systems.
    UpdateProvenance {
        /// Provenance data from source resources organized by node ID
        source_prov: HashMap<String, HashSet<Resource>>,
        /// Destination resource receiving the data and provenance updates
        destination: Resource,
    },

    /// Broadcast Deletion of a resource
    ///
    /// Broadcast the deletion of a resource to all middleware instances.
    BroadcastDeletion(Resource),
}

/// Middleware-to-Middleware (M2M) response types.
///
/// Responses sent between middleware instances for distributed coordination and policy exchange.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum M2mResponse {
    /// Compliance policies for requested source resources.
    ///
    /// Maps each resource to its current policy, enabling the requesting middleware
    /// to evaluate flow compliance before authorization.
    SourceCompliance(HashMap<Resource, Policy>),

    /// Compliance policy for a specific destination resource.
    ///
    /// Contains the current policy that will be applied to incoming data flows
    /// to the requested destination resource.
    DestinationCompliance(Policy),

    /// Acknowledgment of successful request processing.
    ///
    /// Confirms that provenance updates or other operations completed successfully.
    Ack,
}

/// Operator-to-Middleware (O2M) request types.
///
/// Administrative requests from external operators, compliance officers, and organizations
/// for policy management and provenance analysis. These operations typically require
/// elevated privileges and are used for governance and audit purposes.
pub enum O2mRequest {
    /// Retrieve current compliance policies for a set of resources.
    ///
    /// Enables operators to audit current policy configurations and verify
    /// compliance with organizational or regulatory requirements.
    GetPolicies(HashSet<Resource>),

    /// Set a complete compliance policy for a specific resource.
    ///
    /// Replaces the existing policy with a new configuration that defines
    /// confidentiality, integrity, and access control requirements.
    SetPolicy {
        /// Target resource to apply the policy to
        resource: Resource,
        /// New policy configuration
        policy: Policy,
    },

    /// Set confidentiality requirements for a specific resource.
    ///
    /// Updates only the confidentiality aspects of the resource's policy,
    /// leaving other policy components unchanged.
    SetConfidentiality {
        /// Target resource to update
        resource: Resource,
        /// New confidentiality policy requirements
        confidentiality: ConfidentialityPolicy,
    },

    /// Set integrity level requirements for a specific resource.
    ///
    /// Updates the minimum integrity level required for data flows involving
    /// this resource, typically on a scale from 0 (lowest) to higher values.
    SetIntegrity {
        /// Target resource to update
        resource: Resource,
        /// Minimum integrity level (higher values indicate stricter requirements)
        integrity: u32,
    },

    /// Mark a resource as deleted for compliance and audit purposes.
    ///
    /// Indicates that the resource has been removed from the system while
    /// preserving its historical provenance for audit trails.
    SetDeleted(Resource),

    /// Broadcast the deletion of a resource to all middleware instances.
    ///
    /// Uses M2M API to broadcast the deletion to all middleware instances.
    BroadcastDeletion(Resource),

    /// Enforce consent for data processing operations on a resource.
    ///
    /// Set the consent flag to enforce consent for data flows, actually requiring explicit permission for each outgoing flow.
    EnforceConsent(Resource),

    /// Give consent decision for a specific data flow operation.
    ///
    /// Updates the consent status for a pending data flow operation.
    SetConsentDecision {
        /// Source resource providing data
        source: Resource,
        /// Destination resource receiving data
        destination: Destination,
        /// Consent decision: true to grant, false to deny
        decision: bool,
    },

    /// Retrieve the complete provenance lineage for a resource.
    ///
    /// Returns all upstream resources and middleware nodes that have contributed
    /// data to the specified resource, enabling full traceability analysis.
    GetReferences(Resource),
}

/// Operator-to-Middleware (O2M) response types.
///
/// Responses sent to operators and administrators following policy management and
/// provenance query requests.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum O2mResponse {
    /// Current compliance policies for the requested resources.
    ///
    /// Maps each requested resource to its current policy configuration,
    /// enabling operators to review and audit policy settings.
    Policies(HashMap<Resource, Policy>),

    /// Acknowledgment of successful policy update or configuration change.
    ///
    /// Confirms that policy modifications, consent grants, or deletion
    /// markings were applied successfully.
    Ack,

    /// Complete provenance lineage for the requested resource.
    ///
    /// Maps node IDs to sets of resources, showing the complete ancestry
    /// and data flow history for traceability analysis.
    References(HashMap<String, HashSet<Resource>>),
}

/// Sequencer service request types.
///
/// Internal API for the sequencer service, which manages resource reservations and flow
/// coordination to prevent race conditions and ensure consistent ordering of operations
/// across the distributed system.
#[derive(Debug, Clone)]
pub enum SequencerRequest {
    /// Reserve exclusive access for a data flow from source to destination.
    ///
    /// Prevents concurrent modifications to the destination resource while
    /// a flow is being processed, ensuring data consistency and atomic operations.
    ReserveFlow {
        /// Source resource providing data
        source: Resource,
        /// Destination resource receiving data
        destination: Resource,
    },

    /// Release a previously reserved flow to allow subsequent operations.
    ///
    /// Must be called after flow completion to free the destination resource
    /// for other operations and maintain system throughput.
    ReleaseFlow {
        /// Destination resource to release from reservation
        destination: Resource,
    },
}

/// Sequencer service response types.
///
/// Responses from the sequencer service confirming flow reservation and release operations.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SequencerResponse {
    /// Confirmation that the requested flow has been successfully reserved.
    ///
    /// Indicates that exclusive access has been granted and the flow operation
    /// may proceed without interference from concurrent operations.
    FlowReserved,

    /// Confirmation that the flow reservation has been successfully released.
    ///
    /// Returns the source and destination resources that were involved in the
    /// flow, if available, for logging and audit purposes.
    FlowReleased {
        /// Source resource that was part of the released flow
        source: Option<Resource>,
        /// Destination resource that was part of the released flow
        destination: Option<Resource>,
    },
}

/// Provenance service request types.
///
/// Internal API for the provenance service, which tracks data lineage and ancestry
/// relationships across resources and operations. Maintains comprehensive audit trails
/// for compliance and data governance purposes.
#[derive(Debug, Clone)]
pub enum ProvenanceRequest {
    /// Retrieve the complete provenance lineage for a resource.
    ///
    /// Returns all upstream resources and nodes that have contributed data
    /// to the specified resource, enabling full traceability analysis.
    GetReferences(Resource),

    /// Record a new data flow relationship between source and destination resources.
    ///
    /// Updates the destination's provenance to include the source resource,
    /// maintaining the complete lineage chain for audit and compliance purposes.
    UpdateProvenance {
        /// Source resource providing data
        source: Resource,
        /// Destination resource receiving data and provenance updates
        destination: Resource,
    },

    /// Update destination provenance with pre-computed source lineage data.
    ///
    /// Used for cross-node flows where source provenance has been collected
    /// from remote middleware instances and needs to be merged with local records.
    UpdateProvenanceRaw {
        /// Pre-computed provenance data organized by source node ID
        source_prov: HashMap<String, HashSet<Resource>>,
        /// Destination resource to receive the provenance updates
        destination: Resource,
    },
}

/// Provenance service response types.
///
/// Responses from the provenance service containing lineage data and update confirmations.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ProvenanceResponse {
    /// Complete provenance lineage for the requested resource.
    ///
    /// Maps node IDs to sets of resources, showing all upstream dependencies
    /// and data sources that have contributed to the resource's current state.
    Provenance(HashMap<String, HashSet<Resource>>),

    /// Confirmation that provenance was successfully updated with new lineage data.
    ///
    /// Indicates that the destination resource's ancestry records now include
    /// the specified source resource or provenance information.
    ProvenanceUpdated,

    /// Notification that no provenance update was needed.
    ///
    /// Returned when the source resource is already included in the destination's
    /// provenance, avoiding duplicate entries in the lineage records.
    ProvenanceNotUpdated,
}

/// Compliance service request types.
///
/// Internal API for the compliance service, which manages policies, evaluates authorization
/// decisions, and enforces organizational and regulatory requirements for data flows.
#[derive(Debug, Clone)]
pub enum ComplianceRequest {
    /// Evaluate whether a proposed data flow complies with all applicable policies.
    ///
    /// Compares source resource policies against destination requirements to determine
    /// if the flow should be authorized. Considers confidentiality, integrity, consent,
    /// and other policy constraints.
    EvalPolicies {
        /// Policies for source resources organized by authority node ID
        source_policies: HashMap<String, HashMap<Resource, Policy>>,
        /// Destination resource receiving the data
        destination: Resource,
    },

    /// Retrieve the current compliance policy for a specific resource.
    ///
    /// Returns the complete policy configuration including confidentiality,
    /// integrity, consent, and deletion status for the requested resource.
    GetPolicy(Resource),

    /// Retrieve current compliance policies for multiple resources.
    ///
    /// Batch operation to efficiently query policy configurations for
    /// multiple resources in a single request.
    GetPolicies(HashSet<Resource>),

    /// Set a complete compliance policy for a specific resource.
    ///
    /// Replaces the existing policy with new configuration that defines
    /// all aspects of compliance requirements for the resource.
    SetPolicy {
        /// Target resource to apply the policy to
        resource: Resource,
        /// New complete policy configuration
        policy: Policy,
    },

    /// Update confidentiality requirements for a specific resource.
    ///
    /// Modifies only the confidentiality aspects of the resource's policy
    /// while preserving other policy components.
    SetConfidentiality {
        /// Target resource to update
        resource: Resource,
        /// New confidentiality policy requirements
        confidentiality: ConfidentialityPolicy,
    },

    /// Update integrity level requirements for a specific resource.
    ///
    /// Sets the minimum integrity level required for operations involving
    /// this resource, typically on a numerical scale.
    SetIntegrity {
        /// Target resource to update
        resource: Resource,
        /// Minimum required integrity level
        integrity: u32,
    },

    /// Mark a resource as deleted for compliance tracking.
    ///
    /// Updates the resource's policy to reflect its deletion status while
    /// maintaining historical records for audit purposes.
    SetDeleted(Resource),

    /// Update consent enforcement for data processing operations on a resource.
    ///
    /// Set or unset consent enforcement for a resource outgoing flows, actually requiring explicit permission for each outgoing flow.
    /// typically for privacy and regulatory compliance.
    EnforceConsent {
        /// Target resource to update consent for
        resource: Resource,
        /// Consent status: true to grant, false to revoke
        consent: bool,
    },
}

/// Compliance service response types.
///
/// Responses from the compliance service containing authorization decisions, policy data,
/// and update confirmations.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ComplianceResponse {
    /// Authorization granted for the requested data flow operation.
    ///
    /// Indicates that all policy evaluations passed and the operation
    /// complies with organizational and regulatory requirements.
    Grant,

    /// Current compliance policy for the requested resource.
    ///
    /// Contains the complete policy configuration including confidentiality,
    /// integrity, consent, and other compliance requirements.
    Policy(Policy),

    /// Current compliance policies for multiple requested resources.
    ///
    /// Maps each requested resource to its current policy configuration
    /// for batch policy queries.
    Policies(HashMap<Resource, Policy>),

    /// Confirmation that a policy update was successfully applied.
    ///
    /// Indicates that policy modifications, consent changes, or other
    /// compliance configuration updates completed successfully.
    PolicyUpdated,

    /// Notification that no policy update was needed.
    ///
    /// Returned when the requested policy change would result in no
    /// actual modification to the current configuration.
    PolicyNotUpdated,
}
