//! Traceability labels management mechanism for data containers.
mod compliance;
mod provenance;
#[cfg(test)]
mod tests;

pub(crate) use compliance::{Compliance, ComplianceLabel, ComplianceSettings};
pub(crate) use provenance::Provenance;
use provenance::ProvenanceLabel;

use crate::{identifier::Identifier, grpc_proto};

/// Traceability labels structure.
///
/// A [`Labels`] structure is instantiated for each declared container.
#[derive(Debug, Clone)]
pub struct Labels {
    compliance: ComplianceLabel,
    provenance: ProvenanceLabel,
}

impl Labels {
    /// Creates a new [`Labels`] object given an [`Identifier`] enum.
    pub fn new(identifier: Identifier) -> Self {
        Labels {
            compliance: ComplianceLabel::new(identifier),
            provenance: ProvenanceLabel::default(),
        }
    }
}

impl From<grpc_proto::Labels> for Labels {
    fn from(labels: grpc_proto::Labels) -> Self {
        Labels {
            compliance: labels.compliance.unwrap().into(),
            provenance: labels
                .provenance
                .into_iter()
                .map(ComplianceLabel::from)
                .collect(),
        }
    }
}

impl From<Labels> for grpc_proto::Labels {
    fn from(labels: Labels) -> Self {
        grpc_proto::Labels {
            compliance: Some(labels.compliance.into()),
            provenance: labels
                .provenance
                .into_iter()
                .map(grpc_proto::ComplianceLabel::from)
                .collect(),
        }
    }
}
