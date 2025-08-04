//! A distributed traceability middleware.
//!
//! This software provides a mediation layer that provides provenance recording
//! and compliance enforcement on I/O objects such as files or streams. The use
//! of a custom I/O library is required to wrap standard I/O library methods to
//! make input/output conditional on middleware authorization.
//!
//! A unique instance of this software should run on each node where
//! traceability is enforced.
//!
//! Process/middleware and middleware/middleware communication relies [`tonic`], a Rust implementation of gRPC, a high performance, open source, general RPC framework on the
//! gRPC framework
//!
//! [`tonic`]: https://docs.rs/tonic

#[cfg(test)]
pub mod tests;

pub mod traceability;
pub mod transport;
