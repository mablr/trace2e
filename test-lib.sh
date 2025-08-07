#!/bin/bash

cargo build --release -p trace2e_middleware
./target/release/trace2e_middleware &
TRACE2E_PID=$!
cargo test -p stde2e
kill ${TRACE2E_PID}