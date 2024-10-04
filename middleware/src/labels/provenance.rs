use crate::identifier::Identifier;

use super::Labels;

pub(crate) type ProvenanceLabel = Vec<Identifier>;

pub(crate) trait Provenance {
    fn update_prov(&mut self, source: &Labels);
    fn set_prov(&mut self, provenance: ProvenanceLabel);
    fn get_prov(&self) -> ProvenanceLabel;
}

impl Provenance for Labels {
    /// Returns the provenance information.
    fn get_prov(&self) -> ProvenanceLabel {
        self.provenance.clone()
    }

    /// Sets the provenance information given remote provenance.
    fn set_prov(&mut self, provenance: ProvenanceLabel) {
        self.provenance = provenance;
    }

    /// Updates the provenance information given a source [`Labels`] object.
    ///
    /// The provenance references of the source object are merged into self
    /// object by avoiding duplicates.  
    fn update_prov(&mut self, source: &Labels) {
        for identifier in &source.provenance {
            if !self.provenance.contains(&identifier) {
                self.provenance.push(identifier.clone());
            }
        }
    }
}
