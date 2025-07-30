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
pub enum M2mRequest {
    GetConsistentCompliance {
        source: Identifier,
        destination: Identifier,
    },
    GetLooseCompliance {
        authority_ip: String,
        ids: HashSet<Identifier>,
    },
    ProvenanceUpdate {
        source_prov: HashSet<Identifier>,
        destination: Identifier,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum M2mResponse {
    Compliance(HashSet<Policy>),
    Ack,
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
    ReleaseFlow { destination: Identifier },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SequencerResponse {
    /// Flow successfully reserved
    FlowReserved,
    /// Flow successfully released
    FlowReleased {
        source: Option<Identifier>,
        destination: Option<Identifier>,
    },
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
    /// Update destination provenance with raw sourceprovenance
    UpdateProvenanceRaw {
        source_prov: HashSet<Identifier>,
        destination: Identifier,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ProvenanceResponse {
    /// Complete provenance set for the requested resource
    Provenance(HashSet<Identifier>),
    /// Provenance successfully updated
    ProvenanceUpdated,
    /// Provenance not updated as the source is already in the destination provenance
    ProvenanceNotUpdated,
}

/// Compliance-specific API for policy management and flow authorization
#[derive(Debug, Clone)]
pub enum ComplianceRequest {
    /// Check if a flow from source to destination is compliant with policies
    CheckCompliance {
        source_policies: HashSet<Policy>,
        destination_policy: Policy,
    },
    /// Get policy for a specific resource
    GetPolicies { ids: HashSet<Identifier> },
    /// Set policy for a specific resource
    SetPolicy { id: Identifier, policy: Policy },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ComplianceResponse {
    /// Flow is compliant and authorized
    Grant,
    /// Policies for the requested resources
    Policies(HashSet<Policy>),
    /// Policy successfully set
    PolicyUpdated,
}
