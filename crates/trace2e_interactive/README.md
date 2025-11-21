# trace2e_interactive

Simple DSL for executing traced I/O operations. Supports interactive and batch (playbook) modes.

## Build

```bash
cargo build -p trace2e_interactive --release
```

Output: `target/release/e2e-proc` and `target/release/e2e-op`

## Usage

### Interactive

```bash
cargo run -p trace2e_interactive --bin e2e-proc
# or
./target/release/e2e-proc
```

### Operator

```bash
cargo run -p trace2e_interactive --bin e2e-op
# or
./target/release/e2e-op
```

## Instruction Syntax

Format: `COMMAND resource`

**Commands:**
- `READ` / `R` - Read from resource
- `WRITE` / `W` - Write to resource
- `HELP` / `H` / `?` - Show help

**Resources:**
- Files: `file:///path` (e.g., `file:///tmp/test.txt`)
- Streams: `stream://local::peer` (e.g., `stream://127.0.0.1:8080::192.168.1.1:9000`)

**Examples:**
```
READ file:///tmp/data.txt
WRITE stream://127.0.0.1:8080::192.168.1.1:9000
R file:///tmp/test.txt
W stream://127.0.0.1:8080::192.168.1.1:9000
```

## Playbook Format

One instruction per line. Supports comments (`#`) and empty lines.

Example (`scenario.txt`):
```
# Open file
OPEN file:///tmp/input.txt

# Read input file
READ file:///tmp/input.txt

# Write output
WRITE file:///tmp/output.txt

# Stream operations (requires at least two nodes)
# READ stream://192.168.1.1:8080::192.168.1.2:9000
# WRITE stream://192.168.1.1:8080::192.168.1.2:9000
```

## Requirements

**Important**: The `stde2e` library requires a local instance of trace2e middleware running at `[::1]:50051`.

See the main README for setup details about `trace2e_middleware`.

## Tests

```bash
cargo test -p trace2e_interactive
```
