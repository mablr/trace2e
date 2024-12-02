//! Traceability labels management mechanism for data containers.
mod compliance;
mod provenance;
#[cfg(test)]
mod tests;

pub(crate) use compliance::{Compliance, ComplianceLabel, ComplianceSettings};
pub(crate) use provenance::Provenance;
use provenance::ProvenanceLabel;

use crate::{identifier::Identifier, m2m_service::m2m};

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

impl From<m2m::Labels> for Labels {
    fn from(labels: m2m::Labels) -> Self {
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

impl From<Labels> for m2m::Labels {
    fn from(labels: Labels) -> Self {
        m2m::Labels {
            compliance: Some(labels.compliance.into()),
            provenance: labels
                .provenance
                .into_iter()
                .map(m2m::ComplianceLabel::from)
                .collect(),
        }
    }
}
