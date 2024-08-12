use crate::containers::ContainerError;

#[derive(Debug, PartialEq)]
pub enum ProvenanceError {
    Inconsistency(ContainerError),
}

impl std::fmt::Display for ProvenanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProvenanceError::Inconsistency(e) => write!(f, "Container '{}' is not registered.", e),
        }
    }
}

impl std::error::Error for ProvenanceError {}
