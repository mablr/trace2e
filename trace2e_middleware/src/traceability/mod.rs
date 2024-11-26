//! Provenance module.
//!
//! This module is used for provenance tracking, it provides a global consistent
//! state at the system level with a fully asynchronous paradigm using message
//! passing.
mod error;
mod message;
mod server;
#[cfg(test)]
mod tests;
mod whyenf;

pub use error::TraceabilityError;
pub use message::{TraceabilityRequest, TraceabilityResponse};
pub use server::traceability_server;
