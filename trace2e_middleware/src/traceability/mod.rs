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

pub use error::TraceabilityError;
pub use message::TraceabilityRequest;
pub use message::TraceabilityResponse;
pub use server::init_traceability_server;

