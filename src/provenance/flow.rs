use crate::identifiers::Identifier;

#[derive(Debug, Clone)]
pub struct Flow {
    pub id: u64,
    pub source: Identifier,
    pub destination: Identifier,
}
