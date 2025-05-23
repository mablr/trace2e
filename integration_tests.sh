#!/bin/bash
set -eux -o pipefail

# Compile Middleware
cargo build --release

# Launch Middleware
./target/release/trace2e_middleware &
TRACE2E_PID=$!

# Synchronous integration tests
cargo test integration_sync -- --test-threads 1

# Async integration tests
cargo test integration_async_mp

# Client integration tests
cargo test integration_client

# Stop Middleware
kill ${TRACE2E_PID}