//! Traceability labels management mechanism for data containers.
mod compliance;
mod provenance;
#[cfg(test)]
mod tests;

pub(crate) use compliance::{Compliance, ConfidentialityLabel};
pub(crate) use provenance::{Provenance, ProvenanceLabel};

use crate::identifier::Identifier;

/// Traceability labels structure.
///
/// A [`Labels`] structure is instantiated for each declared container.
#[derive(Debug, Clone)]
pub struct Labels {
    provenance: ProvenanceLabel,
    confidentiality: ConfidentialityLabel,
}

impl Labels {
    /// Creates a new [`Labels`] object given an [`Identifier`] enum, and a 
    /// confidentiality level reprensented by [`ConfidentialityLabel`] enum.
    ///
    /// If the object is instantiated for a File [`Identifier`] container, the
    /// provenance is initialized with the given Identifier.
    pub fn new(identifier: Identifier, confidentiality: ConfidentialityLabel) -> Self {
        let mut labels = Labels {
            provenance: Vec::new(),
            confidentiality,
        };

        if identifier.is_file().is_some() {
            labels.provenance.push(identifier);
        }

        labels
    }
}
