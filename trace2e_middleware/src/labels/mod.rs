//! Traceability labels management mechanism for data containers.
mod compliance;
mod provenance;
#[cfg(test)]
mod tests;

pub(crate) use compliance::{Compliance, ComplianceLabel};
pub(crate) use provenance::Provenance;
use provenance::ProvenanceLabel;

use crate::identifier::Identifier;

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
