use super::Labels;
use crate::{identifier::Identifier, m2m_service::m2m};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplianceLabel {
    identifier: Identifier,
    local_confidentiality: bool,
    local_integrity: bool,
}

impl ComplianceLabel {
    pub fn new(identifier: Identifier) -> Self {
        Self {
            identifier,
            local_confidentiality: false,
            local_integrity: false,
        }
    }
}

impl From<m2m::ComplianceLabel> for ComplianceLabel {
    fn from(cl: m2m::ComplianceLabel) -> Self {
        ComplianceLabel {
            identifier: cl.identifier.unwrap().into(),
            local_confidentiality: cl.local_confidentiality,
            local_integrity: cl.local_integrity,
        }
    }
}

impl From<ComplianceLabel> for m2m::ComplianceLabel {
    fn from(cl: ComplianceLabel) -> Self {
        m2m::ComplianceLabel {
            identifier: Some(cl.identifier.into()),
            local_confidentiality: cl.local_confidentiality,
            local_integrity: cl.local_confidentiality,
        }
    }
}

pub trait Compliance {
    fn is_compliant(&self, source: &Self) -> bool;
    fn get_identifier(&self) -> &Identifier;
}

impl Compliance for Labels {
    fn is_compliant(&self, source: &Self) -> bool {
        self.enforce_confidentiality(source) && self.enforce_integrity(source)
    }

    fn get_identifier(&self) -> &Identifier {
        &self.compliance.identifier
    }
}

pub trait ComplianceSettings {
    fn get_local_confidentiality(&self) -> bool;
    fn get_local_integrity(&self) -> bool;
    fn set_local_confidentiality(&mut self, value: bool);
    fn set_local_integrity(&mut self, value: bool);
}

impl ComplianceSettings for Labels {
    fn get_local_confidentiality(&self) -> bool {
        self.compliance.local_confidentiality
    }

    fn get_local_integrity(&self) -> bool {
        self.compliance.local_integrity
    }

    fn set_local_confidentiality(&mut self, value: bool) {
        self.compliance.local_confidentiality = value;
    }

    fn set_local_integrity(&mut self, value: bool) {
        self.compliance.local_integrity = value;
    }
}

trait ComplianceEnforcement {
    fn enforce_confidentiality(&self, source: &Self) -> bool;
    fn enforce_integrity(&self, source: &Self) -> bool;
}

impl ComplianceEnforcement for Labels {
    fn enforce_confidentiality(&self, source: &Self) -> bool {
        if self.compliance.identifier.is_stream().is_some() {
            !(source.compliance.local_confidentiality
                || source
                    .provenance
                    .iter()
                    .filter(|cl| cl.identifier.is_local())
                    .any(|cl| cl.local_confidentiality))
        } else {
            true
        }
    }

    fn enforce_integrity(&self, source: &Self) -> bool {
        if self.compliance.local_integrity {
            source.compliance.identifier.is_local()
                && source.provenance.iter().all(|cl| cl.identifier.is_local())
        } else {
            true
        }
    }
}
