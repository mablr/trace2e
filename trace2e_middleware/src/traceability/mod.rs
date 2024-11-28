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
pub(crate) use message::TraceabilityResponse;
pub use server::traceability_server;
