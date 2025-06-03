use super::naming::Identifier;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default, Clone)]
pub struct Provenance {
    derived_from_map: HashMap<Identifier, HashSet<Identifier>>,
}

impl Provenance {
    pub fn enroll(&mut self, id: Identifier) {
        if !self.derived_from_map.contains_key(&id) {
            let mut derived_from = HashSet::new();
            derived_from.insert(id.clone());
            self.derived_from_map.insert(id.clone(), derived_from);
        }
    }

    pub fn get_prov(&self, id: Identifier) -> Option<HashSet<Identifier>> {
        self.derived_from_map.get(&id).cloned()
    }

    pub fn get_prov_mut(&mut self, id: Identifier) -> Option<&mut HashSet<Identifier>> {
        self.derived_from_map.get_mut(&id)
    }

    pub fn update(&mut self, source: Identifier, destination: Identifier) {
        if let (Some(source_prov), Some(destination_prov)) =
            (self.get_prov(source), self.get_prov_mut(destination))
        {
            for id in source_prov {
                destination_prov.insert(id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::traceability::naming::Resource;

    use super::*;

    #[test]
    fn unit_provenance_update_simple() {
        let mut provenance = Provenance::default();
        let node_id = "test".to_string();
        let process = Identifier::new(node_id.clone(), Resource::new_process(0));
        let file = Identifier::new(node_id.clone(), Resource::new_file("/tmp/test".to_string()));

        provenance.enroll(process.clone());
        provenance.enroll(file.clone());

        assert_eq!(
            provenance.get_prov(file.clone()),
            Some(HashSet::from([file.clone()]))
        );
        assert_eq!(
            provenance.get_prov(process.clone()),
            Some(HashSet::from([process.clone()]))
        );

        provenance.update(process.clone(), file.clone());

        // Check that the file is now derived from the process
        assert!(
            provenance
                .get_prov(file.clone())
                .unwrap()
                .is_superset(&provenance.get_prov(process.clone()).unwrap())
        );
    }

    #[test]
    fn unit_provenance_update_circular() {
        let mut provenance = Provenance::default();
        let node_id = "test".to_string();
        let process = Identifier::new(node_id.clone(), Resource::new_process(0));
        let file = Identifier::new(node_id.clone(), Resource::new_file("/tmp/test".to_string()));

        provenance.enroll(process.clone());
        provenance.enroll(file.clone());

        provenance.update(process.clone(), file.clone());
        provenance.update(file.clone(), process.clone());

        // Check the proper handling of circular dependencies
        assert_eq!(
            provenance.get_prov(file.clone()).unwrap(),
            provenance.get_prov(process.clone()).unwrap()
        );
    }
}
