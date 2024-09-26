#!/bin/bash
set -eux -o pipefail

# Compile Middleware
cargo build

# Unit tests
cargo test unit

# Launch Middleware
./target/debug/middleware &
TRACE2E_PID=$!

# Synchronous integration tests
cargo test integration_sync -- --test-threads 1

# Async integration tests
cargo test integration_async

# Stop Middleware
kill ${TRACE2E_PID}