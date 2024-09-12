use crate::identifier::Identifier;

/// Provenance layer error type.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ProvenanceError {
    /// Flow declaration failed because at least one of the given containers is
    /// not registered so far in the middleware.
    MissingRegistration(Identifier, Identifier),
    /// Flow Declaration failed because the type of the given containers is
    /// not valid.
    InvalidFlow(Identifier, Identifier),
    /// Flow recording failed due to missing declaration/grant.
    RecordingFailure(u64),
}

impl std::fmt::Display for ProvenanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProvenanceError::MissingRegistration(id1, id2) => {
                write!(
                    f,
                    "Provenance error: ({:?} || {:?}) are not registered.",
                    id1, id2
                )
            }
            ProvenanceError::InvalidFlow(id1, id2) => {
                write!(
                    f,
                    "Provenance error: {:?}<->{:?} Flow is invalid.",
                    id1, id2
                )
            }
            ProvenanceError::RecordingFailure(grant_id) => {
                write!(f, "Provenance error: unable to record Flow {}.", grant_id)
            }
        }
    }
}

impl std::error::Error for ProvenanceError {}
