use crate::identifiers::Identifier;

#[derive(Debug, PartialEq, Eq)]
pub enum ContainerError {
    NotRegistered(Identifier),
    NotReserved(Identifier),
}

impl std::fmt::Display for ContainerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContainerError::NotRegistered(id) => write!(f, "Container '{}' is not registered.", id),
            ContainerError::NotReserved(id) => write!(f, "Container '{}' is not reserved.", id),
        }
    }
}

impl std::error::Error for ContainerError {}
