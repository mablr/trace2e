//! Provenance module.
//! 
//! This module is used for provenance tracking, it provides a global consistent 
//! state at the system level with a fully asynchronous paradigm using message 
//! passing. 
mod error;
mod layer;
#[cfg(test)]
mod tests;

pub use error::ProvenanceError;
pub use layer::{provenance_layer, ProvenanceAction, ProvenanceResult};
