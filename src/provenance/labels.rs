use crate::identifiers::Identifier;

#[derive(Debug, Clone)]
pub struct Labels {
    provenance: Vec<Identifier>
}

impl Labels {
    pub fn new(identifier: Identifier) -> Self {
        let mut labels = Labels {
            provenance: Vec::new(),
        };
        
        if let Identifier::File(_) = identifier {
            labels.provenance.push(identifier);
        }

        labels
    }

    pub fn get_prov(&self) -> Vec<Identifier> {
        self.provenance.clone()
    }

    pub fn update_prov(&mut self, source: &Labels) {
        for identifier in &source.provenance {
            if !self.provenance.contains(&identifier) {
                self.provenance.push(identifier.clone());
            }
        }
    }
}
