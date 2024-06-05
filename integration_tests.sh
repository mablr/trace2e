#!/bin/bash
set -eux -o pipefail

# Compile Middleware
cargo build

# Launch Middleware
./target/debug/trace2e &
TRACE2E_PID=$!

# Single thread tests
cargo test integration_1p -- --test-threads 1

# Multiple threads tests
cargo test integration_mp

# Stop Middleware
kill ${TRACE2E_PID}