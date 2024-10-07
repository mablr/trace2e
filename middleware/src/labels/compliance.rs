use super::Labels;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfidentialityLabel {
    Low,
    High,
}

pub trait Compliance {
    fn is_compliant(&self, destination: Self) -> bool;
}

impl Compliance for Labels {
    fn is_compliant(&self, destination: Self) -> bool {
        match self.confidentiality {
            ConfidentialityLabel::Low => true,
            ConfidentialityLabel::High => destination.confidentiality == ConfidentialityLabel::High,
        }
    }
}
