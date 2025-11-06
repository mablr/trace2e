//! Structured logging setup for e2e demos
//!
//! This module configures tracing with node_id context to enable log filtering
//! and organization by node.

use std::sync::Once;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

static INIT: Once = Once::new();

/// Initialize tracing for the demo with a specific node ID
///
/// This sets up a tracing subscriber with:
/// - Colored output for terminal readability
/// - Env filter from RUST_LOG (defaults to "trace2e=debug,demos=debug")
/// - Compact formatting
pub fn init_tracing_for_node(node_id: &str) {
    INIT.call_once(|| {
        let filter = EnvFilter::try_from_default_env()
            .or_else(|_| EnvFilter::try_new("off"))
            .unwrap();

        tracing_subscriber::registry()
            .with(filter)
            .with(
                fmt::layer()
                    .with_ansi(true)
                    .compact()
                    .with_target(true)
                    .with_thread_ids(false)
                    .pretty(),
            )
            .init();

        tracing::info!(node_id = %node_id, "=== Node initialized ===");
    });
}
