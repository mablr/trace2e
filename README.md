
# TracE2E - Distributed Traceability Middleware

A distributed traceability framework that provides provenance recording and compliance enforcement for processes' I/O operations in distributed systems.

## Overview

TracE2E framework is composed of several key components:

- **trace2e_middleware**: The core middleware service that handles provenance recording and compliance enforcement
- **trace2e_client**: Client library to interact with the middleware
- **stde2e**: Standard library wrapper providing TracE2E integration using `trace2e_client`
- **tokioe2e**: We propose a patch for `tokio` library providing TracE2E integration using `trace2e_client` like for `stde2e`
- **trace2e_interactive**: Interactive process and operator CLI tools for executing I/O operations and interacting with the middleware.


## Key Features

TracE2E provides a comprehensive suite of capabilities to enable end-to-end provenance recording and compliance enforcement for I/O operations in distributed systems:

- Process Observation: Track and Mediate file and network I/O operations seamlessly at the process level.
- Distributed Provenance Tracking: Provide explainability and accountability across distributed systems with a unified data flow model.
- Compliance Enforcement: policies can be applied to resources taking into account the provenance, the type, and the location of the resource.

## Architecture

The system works through three main components:

1. **Middleware Service**: Runs on each node where traceability is enforced, providing:
   - Process-to-Middleware (P2M) communication
   - Middleware-to-Middleware (M2M) communication
   - Traceability enforcement

2. **Client Library**: Wraps standard I/O operations to:
   - Process-to-Middleware (P2M) communication
   - Register files and network streams opened by the process
   - Request authorization for I/O operations
   - Report operation results

3. **I/O Library Wrapper and Patches**: to provide TracE2E integration using `trace2e_client`:
   - `stde2e`: Standard library wrapper
   - `tokioe2e`: Patch for `tokio` library

## Getting Started

### Prerequisites

- Rust
- Protocol Buffers compiler

### Building from Source

1. Clone the repository:
   ```bash
   git clone https://github.com/mablr/trace2e
   cd trace2e
   ```

2. Build the middleware in release mode to enable optimizations:
   ```bash
   make release
   ./target/release/trace2e_middleware
   ```
The middleware will listen on `[::]:8080` by default.

### Testing

- Run all tests (middleware + custom lib):
   ```bash
   make test
   ```
- Run only middleware tests:
   ```bash
   make test-middleware
   ```
- Run only custom lib tests (wrapper script):
   ```bash
   make test-lib
   ```

### Documentation

This will build the documentation and automatically open it in your default web browser.
```bash
make docs
```

## Project Structure

- **/proto**: Protocol buffer definitions for communication between components
- **/crates/trace2e_core**: Core traceability library of the middleware.
- **/crates/trace2e_middleware**: Middleware binary implementation
- **/crates/trace2e_client**: Middleware client communication library implementation
- **/crates/stde2e**: Standard library wrapper providing TracE2E integration using `trace2e_client`
- **/crates/trace2e_interactive**: Interactive tools for testing and managing TracE2E:
  - **Interactive Process** (`e2e-proc`/`std-proc`): CLI for executing traced I/O operations via commands or playbook files
  - **Operator** (`e2e-op`/`std-op`): CLI for managing compliance policies, consent, and provenance queries through the Operator-to-Middleware (O2M) API
- **/patches**: TracE2E integration patches for various frameworks


## License

This project is licensed under the terms found in the LICENSE file in the root directory.
