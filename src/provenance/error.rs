use crate::containers::ContainerError;

#[derive(Debug, PartialEq, Eq)]
pub enum ProvenanceError {
    ContainerFailure(ContainerError),
}

impl std::fmt::Display for ProvenanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProvenanceError::ContainerFailure(e) => write!(f, "Provenance error: {}", e),
        }
    }
}

impl std::error::Error for ProvenanceError {}
