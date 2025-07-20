use std::collections::HashSet;

use super::{layers::compliance::Policy, naming::Identifier};

#[derive(Debug, Clone)]
pub enum P2mRequest {
    LocalEnroll {
        pid: i32,
        fd: i32,
        path: String,
    },
    RemoteEnroll {
        pid: i32,
        fd: i32,
        local_socket: String,
        peer_socket: String,
    },
    IoRequest {
        pid: i32,
        fd: i32,
        output: bool,
    },
    IoReport {
        pid: i32,
        fd: i32,
        grant_id: u128,
        result: bool,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum P2mResponse {
    Grant(u128), // <- P2mRequest::IoRequest
    Ack,         // <- P2mRequest::{LocalEnroll, RemoteEnroll, Report}
}

#[derive(Debug, Clone)]
pub enum TraceabilityRequest {
    Request {
        // -> TraceabilityResponse::Grant
        source: Identifier,
        destination: Identifier,
    },
    Report {
        // -> TraceabilityResponse::Ack
        source: Identifier,
        destination: Identifier,
        success: bool,
    },
    SetPolicy {
        // -> TraceabilityResponse::Ack
        id: Identifier,
        policy: Policy,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TraceabilityResponse {
    Grant,
    Ack,
    Wait, // Only produced by SequencerService and mapped to Ack by WaitingQueueService
}

/// Sequencer-specific API for resource management and flow control
#[derive(Debug, Clone)]
pub enum SequencerRequest {
    /// Reserve a flow from source to destination
    ReserveFlow {
        source: Identifier,
        destination: Identifier,
    },
    /// Release a flow from source to destination  
    ReleaseFlow {
        source: Identifier,
        destination: Identifier,
    },
    /// Check if resources are available for a flow
    CheckAvailability {
        source: Identifier,
        destination: Identifier,
    },
    /// Get current flow status
    GetFlowStatus {
        source: Identifier,
        destination: Identifier,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SequencerResponse {
    /// Flow successfully reserved
    FlowReserved,
    /// Flow successfully released, and notify there is a waiting queue
    FlowReleased { notify: bool },
    /// Source resource is unavailable (already reserved by another writer)
    SourceUnavailable,
    /// Destination resource is unavailable (already reserved)
    DestinationUnavailable,
    /// Both source and destination are unavailable
    BothUnavailable,
}

/// Provenance-specific API for lineage tracking and data provenance
#[derive(Debug, Clone)]
pub enum ProvenanceRequest {
    /// Get the complete provenance (lineage) of a resource
    GetProvenance { id: Identifier },
    /// Update provenance when data flows from source to destination
    UpdateProvenance {
        source: Identifier,
        destination: Identifier,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ProvenanceResponse {
    /// Complete provenance set for the requested resource
    Provenance { lineage: HashSet<Identifier> },
    /// Provenance successfully updated
    ProvenanceUpdated,
}

/// Compliance-specific API for policy management and flow authorization
#[derive(Debug, Clone)]
pub enum ComplianceRequest {
    /// Check if a flow from source to destination is compliant with policies
    CheckFlowCompliance {
        source: Identifier,
        destination: Identifier,
    },
    /// Set policy for a specific resource
    SetPolicy { id: Identifier, policy: Policy },
    /// Get policy for a specific resource
    GetPolicy { id: Identifier },
    /// Remove policy for a specific resource
    RemovePolicy { id: Identifier },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ComplianceResponse {
    /// Flow is compliant and authorized
    FlowAuthorized,
    /// Flow violates policy and is denied
    FlowDenied,
    /// Policy successfully set
    PolicySet,
    /// Policy information
    Policy { policy: Policy },
    /// Policy successfully removed
    PolicyRemoved,
}
