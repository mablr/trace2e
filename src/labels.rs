//! Traceability labels management mechanism for data containers.

use crate::identifier::Identifier;

/// Traceability labels structure.
///
/// A [`Labels`] structure is instantiated for each declared container.
#[derive(Debug, Clone)]
pub struct Labels {
    provenance: Vec<Identifier>,
}

impl Labels {
    /// Creates a new [`Labels`] object given an [`Identifier`] enum.
    ///
    /// If the object is instantiated for a File [`Identifier`] container, the
    /// provenance is initialized with the given Identifier.
    pub fn new(identifier: Identifier) -> Self {
        let mut labels = Labels {
            provenance: Vec::new(),
        };

        if identifier.is_file().is_some() {
            labels.provenance.push(identifier);
        }

        labels
    }

    /// Returns the provenance information.
    pub fn get_prov(&self) -> Vec<Identifier> {
        self.provenance.clone()
    }

    /// Sets the provenance information given remote provenance.
    pub fn set_prov(&mut self, provenance: Vec<Identifier>) {
        self.provenance = provenance;
    }

    /// Updates the provenance information given a source [`Labels`] object.
    ///
    /// The provenance references of the source object are merged into self
    /// object by avoiding duplicates.  
    pub fn update_prov(&mut self, source: &Labels) {
        for identifier in &source.provenance {
            if !self.provenance.contains(&identifier) {
                self.provenance.push(identifier.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use super::*;

    #[test]
    fn unit_labels_new() {
        let id1 = Identifier::new_process(1, 1);
        let id2 = Identifier::new_file("/path/to/file1.txt".to_string());
        let id3 = Identifier::new_stream(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
        );

        let id1_labels = Labels::new(id1.clone());
        let id2_labels = Labels::new(id2.clone());
        let id3_labels = Labels::new(id3.clone());

        assert_eq!(id1_labels.get_prov(), vec![]);
        assert_eq!(id2_labels.get_prov(), vec![id2.clone()]);
        assert_eq!(id3_labels.get_prov(), vec![]);
    }

    #[test]
    fn unit_labels_set() {
        let id1 = Identifier::new_process(1, 1);
        let id2 = Identifier::new_file("/path/to/file1.txt".to_string());
        let id3 = Identifier::new_stream(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
        );

        let mut labels = Labels::new(id1.clone());
        assert_eq!(labels.get_prov(), vec![]);

        labels.set_prov(vec![id2.clone(), id3.clone()]);
        assert_eq!(labels.get_prov(), vec![id2.clone(), id3.clone()]);
    }

    #[test]
    fn unit_labels_update() {
        let id1 = Identifier::new_process(1, 1);
        let id2 = Identifier::new_file("/path/to/file1.txt".to_string());
        let id3 = Identifier::new_file("/path/to/file2.txt".to_string());

        let mut id1_labels = Labels::new(id1.clone());
        let id2_labels = Labels::new(id2.clone());
        let mut id3_labels = Labels::new(id3.clone());

        id1_labels.update_prov(&id2_labels);
        id3_labels.update_prov(&id1_labels);

        assert_eq!(id1_labels.get_prov(), vec![id2.clone()]);
        assert_eq!(id2_labels.get_prov(), vec![id2.clone()]);
        assert_eq!(id3_labels.get_prov(), vec![id3.clone(), id2.clone()]);
    }
}
