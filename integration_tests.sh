#!/bin/bash
set -eux -o pipefail

# Compile Middleware
cargo build

# Launch Middleware
./target/debug/trace2e &
TRACE2E_PID=$!

# Tests
cargo test

# Stop Middleware
kill ${TRACE2E_PID}