use super::{compliance::ComplianceLabel, Compliance, Labels};

pub(super) type ProvenanceLabel = Vec<ComplianceLabel>;

pub trait Provenance {
    fn update_prov(&mut self, source: &Labels);
    fn clear_prov(&mut self);
    fn set_prov(&mut self, provenance: Vec<ComplianceLabel>);
    fn get_prov(&self) -> Vec<ComplianceLabel>;
}

impl Provenance for Labels {
    /// Returns the provenance information as the list of Identifiers references.
    fn get_prov(&self) -> Vec<ComplianceLabel> {
        self.provenance.clone()
    }

    /// Sets the provenance information given remote provenance.
    fn set_prov(&mut self, provenance: Vec<ComplianceLabel>) {
        self.provenance = provenance;
    }

    /// Updates the provenance information given a source [`Labels`] object.
    ///
    /// The provenance references of the source object are merged into self
    /// object by avoiding duplicates.
    fn update_prov(&mut self, source: &Labels) {
        if !(self.provenance.contains(&source.compliance) || self.compliance == source.compliance) {
            self.provenance.push(source.compliance.clone());
        }
        for cl in &source.provenance {
            if !(self.provenance.contains(cl) || self.compliance == *cl) {
                self.provenance.push(cl.clone());
            }
        }
    }

    /// Clears the provenance information, it has effect only streams.
    fn clear_prov(&mut self) {
        if self.get_identifier().is_stream().is_some() {
            self.provenance = Vec::new();
        }
    }
}
