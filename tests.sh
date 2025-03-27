#!/bin/bash
set -eux -o pipefail

# Compile Middleware
cargo build

# Unit tests
cargo test unit

# Launch Middleware
./target/debug/trace2e_middleware &
TRACE2E_PID=$!

# Synchronous integration tests
cargo test integration_sync -- --test-threads 1

# Async integration tests
cargo test integration_async_mp

# Stop Middleware
kill ${TRACE2E_PID}