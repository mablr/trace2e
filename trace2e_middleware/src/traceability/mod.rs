//! Provenance module.
//!
//! This module is used for provenance tracking, it provides a global consistent
//! state at the system level with a fully asynchronous paradigm using message
//! passing.
mod client;
mod error;
mod message;
mod server;
#[cfg(test)]
mod tests;

pub use client::TraceabilityClient;
pub use error::TraceabilityError;
pub use message::TraceabilityRequest;
pub use message::TraceabilityResponse;
pub use server::spawn_traceability_server;
