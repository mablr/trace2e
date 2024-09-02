//! Provenance layer module.

mod error;
mod layer;
#[cfg(test)]
mod tests;

pub use error::ProvenanceError;
pub use layer::{provenance_layer, ProvenanceAction, ProvenanceResult};
