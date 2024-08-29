use crate::identifiers::Identifier;

#[derive(Debug, PartialEq, Eq)]
pub enum ProvenanceError {
    MissingRegistration(Identifier, Identifier),
    InvalidFlow(Identifier, Identifier),
    RecordingFailure(u64),
}

impl std::fmt::Display for ProvenanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProvenanceError::MissingRegistration(id1, id2) => {
                write!(
                    f,
                    "Provenance error: ({} || {}) are not registered.",
                    id1, id2
                )
            }
            ProvenanceError::InvalidFlow(id1, id2) => {
                write!(f, "Provenance error: {}<->{} Flow is invalid.", id1, id2)
            }
            ProvenanceError::RecordingFailure(grant_id) => {
                write!(f, "Provenance error: unable to record Flow {}.", grant_id)
            }
        }
    }
}

impl std::error::Error for ProvenanceError {}
