use super::Labels;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfidentialityLabel {
    Low,
    High,
}

pub trait Compliance {
    fn is_compliant(&self, source: Self) -> bool;
}

impl Compliance for Labels {
    fn is_compliant(&self, source: Self) -> bool {
        match self.confidentiality {
            ConfidentialityLabel::Low => source.confidentiality == ConfidentialityLabel::Low,
            ConfidentialityLabel::High => true,
        }
    }
}
