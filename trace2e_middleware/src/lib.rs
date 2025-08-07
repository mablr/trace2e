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

#[cfg(feature = "trace2e_tracing")]
pub mod trace2e_tracing {
    use std::sync::Once;
    use tracing_subscriber::{EnvFilter, fmt};

    static INIT: Once = Once::new();

    /// Initialize tracing for tests
    /// This sets up a tracing subscriber that will display logs during test execution.
    /// Call this at the beginning of tests that need to see tracing output.
    pub fn init() {
        INIT.call_once(|| {
            let filter = EnvFilter::try_from_default_env()
                .or_else(|_| EnvFilter::try_new("off"))
                .unwrap();

            fmt()
                .with_target(false)
                .with_test_writer()
                .with_env_filter(filter)
                .init();
        });
    }
}
