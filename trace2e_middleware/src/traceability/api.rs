use std::collections::{HashMap, HashSet};

use crate::traceability::naming::Resource;

use super::core::compliance::Policy;

#[derive(Debug, Clone)]
pub enum P2mRequest {
    /// Declare a file opened by a process
    LocalEnroll {
        pid: i32,
        fd: i32,
        path: String,
    },
    /// Declare a stream opened by a process
    RemoteEnroll {
        pid: i32,
        fd: i32,
        local_socket: String,
        peer_socket: String,
    },
    /// Request a flow between a process and a file or stream
    IoRequest {
        pid: i32,
        fd: i32,
        output: bool,
    },
    /// Report the result of a flow between a process and a file or stream
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
    /// Get the consistent compliance of a remote resource
    GetConsistentCompliance {
        source: Resource,
        destination: Resource,
    },
    /// Get the loose compliance of a remote resource
    GetLooseCompliance {
        authority_ip: String,
        resources: HashSet<Resource>,
    },
    /// Update the provenance of a remote resource
    UpdateProvenance {
        source_prov: HashMap<String, HashSet<Resource>>,
        destination: Resource,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum M2mResponse {
    Compliance(HashSet<Policy>),
    Ack,
}

pub enum O2mRequest {
    /// Get policies for a specific set of resources
    GetPolicies(HashSet<Resource>),
    /// Set policy for a specific resource
    SetPolicy { resource: Resource, policy: Policy },
    /// Get the complete provenance (lineage) of a resource
    GetReferences(Resource),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum O2mResponse {
    /// Policies for the requested resources
    Policies(HashSet<Policy>),
    /// Policy successfully set
    Ack,
    /// Provenance set for the requested resource
    References(HashMap<String, HashSet<Resource>>),
}

/// Sequencer-specific API for resource management and flow control
#[derive(Debug, Clone)]
pub enum SequencerRequest {
    /// Reserve a flow from source to destination
    ReserveFlow {
        source: Resource,
        destination: Resource,
    },
    /// Release a flow from source to destination
    ReleaseFlow { destination: Resource },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SequencerResponse {
    /// Flow successfully reserved
    FlowReserved,
    /// Flow successfully released
    FlowReleased {
        source: Option<Resource>,
        destination: Option<Resource>,
    },
}

/// Provenance-specific API for lineage tracking and data provenance
#[derive(Debug, Clone)]
pub enum ProvenanceRequest {
    /// Get the references of a resource
    GetReferences(Resource),
    /// Update provenance when data flows from source to destination
    UpdateProvenance {
        source: Resource,
        destination: Resource,
    },
    /// Update destination provenance with raw source provenance
    UpdateProvenanceRaw {
        source_prov: HashMap<String, HashSet<Resource>>,
        destination: Resource,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ProvenanceResponse {
    /// Provenance set for the requested resource
    Provenance {
        authority: String,
        references: HashMap<String, HashSet<Resource>>,
    },
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
        source_policies: HashMap<String, HashSet<Policy>>,
        destination_policy: Policy,
    },
    /// Get policies for a specific set of resources
    GetPolicies(HashSet<Resource>),
    /// Set policy for a specific resource
    SetPolicy { resource: Resource, policy: Policy },
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
