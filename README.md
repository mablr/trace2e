# TracE2E - Distributed Traceability Middleware

A distributed traceability framework that provides provenance recording and compliance enforcement for processes' I/O operations in distributed systems.

## Overview

TracE2E framework is composed of several key components:

- **trace2e_middleware**: The core middleware service that handles provenance recording and compliance enforcement
- **trace2e_client**: Client library to interact with the middleware
- **stde2e**: Standard library wrapper providing TracE2E integration using `trace2e_client`
- **tokioe2e**: We propose a patch for `tokio` library providing TracE2E integration using `trace2e_client` like for `stde2e`


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

## Getting Started

### Prerequisites

- Rust
- Protocol Buffers compiler
- Docker and Docker Compose (for running demos)

### Quick Demo

The easiest way to see TracE2E in action is to run the Hyper.rs demo in the [demo/hyper](demo/hyper) directory. This demo showcases:

- A web server with TracE2E integration
- File traceability
- Confidentiality enforcement
- Integrity enforcement

Follow the instructions in [demo/hyper/README.md](demo/hyper/README.md) to run the demo.

### Building from Source

1. Clone the repository:
   ```bash
   git clone https://github.com/mablr/trace2e
   cd trace2e
   ```

2. Build the middleware in release mode to enable optimizations:
   ```bash
   cargo build -r
   ./target/release/trace2e_middleware
   ```
The middleware will listen on `[::]:8080` by default.

### Benchmarking

To benchmark the middleware internal traceability server, run the following command:
```bash
cargo bench -p trace2e_middleware
```

To benchmark the gRPC service and the standard library wrapper, run the following commands:
```bash
./target/release/trace2e_middleware &
cargo bench -p trace2e_client
cargo bench -p stde2e
```

## Project Structure

- **/proto**: Protocol buffer definitions for communication between components
- **/trace2e_middleware**: Core middleware implementation
- **/trace2e_client**: Middleware client communication library implementation
- **/stde2e**: Standard library wrapper providing TracE2E integration using `trace2e_client`
- **/demo**: Example applications showcasing TracE2E usage
- **/patches**: TracE2E integration patches for various frameworks


## License

This project is licensed under the terms found in the LICENSE file in the root directory.
