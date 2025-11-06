//! Trace2e E2E Demos - Shared Utilities
//!
//! This library provides common utilities for the e2e demonstration binaries,
//! including structured logging, resource orchestration, and test fixtures.

#[macro_use]
pub mod macros;

pub mod logging;
pub mod orchestration;

// Re-export types from orchestration
pub use orchestration::{FileMapping, StreamMapping};
