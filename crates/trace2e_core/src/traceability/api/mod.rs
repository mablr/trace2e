//! External-facing APIs for the traceability system.
//!
//! This module provides the three main API layers for interacting with the
//! traceability middleware:
//!
//! - **Process-to-Middleware (P2M)**: For application processes to register resources
//!   and request I/O operations with traceability guarantees
//! - **Middleware-to-Middleware (M2M)**: For distributed middleware coordination,
//!   policy exchange, and provenance synchronization
//! - **Operator-to-Middleware (O2M)**: For administrative policy management and
//!   provenance analysis by operators

pub mod m2m;
pub mod o2m;
pub mod p2m;
pub mod types;

// Re-export all types for convenience
pub use types::*;
