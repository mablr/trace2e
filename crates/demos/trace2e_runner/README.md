# trace2e_runner

A simple instruction interpreter for executing traced I/O scenarios using the `stde2e` library.

## Overview

`trace2e_runner` provides a DSL (Domain Specific Language) for scripting file and network I/O operations that are traced through the trace2e system. It supports both interactive (REPL) and batch (script file) execution modes.

## Installation

Build the binary:

```bash
cargo build -p trace2e_runner --release
```

The binary will be available at `target/release/trace2e-runner`.

## Usage

### Interactive Mode

Run the interpreter without arguments to enter interactive mode:

```bash
cargo run -p trace2e_runner
```

You'll see a prompt where you can enter commands:

```
trace2e-runner - Interactive Mode
==================================

Available commands:
  OPEN file:///path@node_id
  OPEN stream://local_socket::peer_socket@node_id
  READ file:///path@node_id
  WRITE stream://local_socket::peer_socket@node_id

Press Ctrl+D to exit

> OPEN file:///tmp/test.txt@127.0.0.1
✓ Opened file: /tmp/test.txt
> READ file:///tmp/test.txt@127.0.0.1
✓ Read 42 bytes from file: /tmp/test.txt
> ^D
Goodbye!
```

### Batch Mode

Execute commands from a script file:

```bash
cargo run -p trace2e_runner -- --file example_scenario.txt
```

Or using the binary directly:

```bash
./target/release/trace2e-runner --file scenario.txt
```

## Command Syntax

### Resource Format

Resources follow the trace2e naming convention:

- **Files**: `file:///path@node_id`
  - Example: `file:///tmp/data.txt@127.0.0.1`

- **Streams**: `stream://local_socket::peer_socket@node_id`
  - Example: `stream://127.0.0.1:8080::192.168.1.1:9000@10.0.0.1`

### Commands

1. **OPEN** - Opens a file or connects to a stream
   ```
   OPEN file:///tmp/test.txt@127.0.0.1
   OPEN stream://127.0.0.1:8080::127.0.0.1:8081@127.0.0.1
   ```

2. **READ** - Reads from a previously opened resource
   ```
   READ file:///tmp/test.txt@127.0.0.1
   READ stream://127.0.0.1:8080::127.0.0.1:8081@127.0.0.1
   ```

3. **WRITE** - Writes test data to a previously opened resource
   ```
   WRITE file:///tmp/output.txt@127.0.0.1
   WRITE stream://127.0.0.1:8080::127.0.0.1:8081@127.0.0.1
   ```

### Script File Format

Script files support:
- One command per line
- Comments starting with `#`
- Empty lines (ignored)

Example (`scenario.txt`):

```
# File operations scenario
OPEN file:///tmp/input.txt@127.0.0.1
READ file:///tmp/input.txt@127.0.0.1

# Write to output
OPEN file:///tmp/output.txt@127.0.0.1
WRITE file:///tmp/output.txt@127.0.0.1

# Stream operations (requires server)
# OPEN stream://127.0.0.1:8080::127.0.0.1:8081@127.0.0.1
# WRITE stream://127.0.0.1:8080::127.0.0.1:8081@127.0.0.1
```

## Integration with trace2e

This runner uses the `stde2e` library directly, which means all I/O operations are automatically traced through the trace2e system.

### Prerequisites

**IMPORTANT**: The `stde2e` library requires the P2M (Policy-to-Manifest) middleware service to be running. The service must be accessible at `[::1]:50051` (the default gRPC endpoint).

Without the P2M service running, the interpreter will panic with a connection error:

```
panicked at crates/trace2e_client/src/p2m.rs:19:66:
called `Result::unwrap()` on an `Err` value: tonic::transport::Error(Transport, ConnectError(...))
```

To run the P2M service, refer to the `trace2e_middleware` crate or use the demo setup from `unattended_deletion_consent` which spawns the required services.

All I/O operations executed by this runner are subject to policy enforcement and provenance tracking configured in the middleware services.

## Testing

Run the unit tests:

```bash
cargo test -p trace2e_runner
```

## Examples

See `example_scenario.txt` for a sample scenario file.

To test file operations:

```bash
# Create a test file
echo "Hello, trace2e!" > /tmp/test_input.txt

# Run the example scenario
cargo run -p trace2e_runner -- --file example_scenario.txt
```

For stream operations, you'll need a server listening on the specified peer socket address.
