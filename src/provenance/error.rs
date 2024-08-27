use crate::identifiers::Identifier;

#[derive(Debug, PartialEq, Eq)]
pub enum ProvenanceError {
    DeclarationFailure(Identifier, Identifier),
    RecordingFailure(u64),
}

impl std::fmt::Display for ProvenanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProvenanceError::DeclarationFailure(source, destination) => write!(
                f,
                "Provenance error: unable to declare a Flow form {} to {}.",
                source, destination
            ),
            ProvenanceError::RecordingFailure(grant_id) => {
                write!(f, "Provenance error: unable to record Flow {}", grant_id)
            }
        }
    }
}

impl std::error::Error for ProvenanceError {}
