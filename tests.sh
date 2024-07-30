#!/bin/bash
set -eux -o pipefail

# Compile Middleware
cargo build

# Unit tests
cargo test containers

# Launch Middleware
./target/debug/trace2e &
TRACE2E_PID=$!

# Single thread integration tests
cargo test integration_1p -- --test-threads 1

# Multiple threads integration tests
cargo test integration_mp

# Stop Middleware
kill ${TRACE2E_PID}