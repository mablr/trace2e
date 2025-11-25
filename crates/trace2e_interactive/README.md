# trace2e_interactive

Simple DSL for executing traced I/O operations in interactive and batch modes.

## Quick Start

### Interactive Mode
```bash
cargo run -p trace2e_interactive --bin e2e-proc
```

### Batch Mode (Playbooks)
```bash
cargo run -p trace2e_interactive --bin e2e-op
```

## Language Specification

### ABNF Grammar

```abnf
instruction    = command [ WSP argument ] CRLF
command        = open-cmd / create-cmd / bind-cmd / connect-cmd /
                 read-cmd / write-cmd / sleep-cmd / help-cmd / empty
open-cmd       = ("OPEN" / "O") WSP file-target
create-cmd     = ("CREATE" / "C") WSP file-target
bind-cmd       = ("BIND" / "B") WSP socket-addr
connect-cmd    = ("CONNECT" / "CN") WSP socket-addr
read-cmd       = ("READ" / "R") WSP io-target
write-cmd      = ("WRITE" / "W") WSP io-target
sleep-cmd      = ("SLEEP" / "S") WSP duration
help-cmd       = "HELP" / "H" / "?"
empty          = [ "#" comment ]

io-target      = file-target / stream-target / socket-target
file-target    = "file://" file-path
socket-target  = "socket://" socket-addr
stream-target  = "stream://" socket-addr "::::" socket-addr
socket-addr    = host ":" port
duration       = DIGIT?

comment        = *VCHAR
file-path      = VCHAR *( VCHAR / WSP )
host           = IPv4address / IPv6address
port           = 1*5DIGIT
```

### Commands

| Command | Aliases | Argument | Description |
|---------|---------|----------|-------------|
| OPEN | O | file-path | Open file for reading/writing |
| CREATE | C | file-path | Create file (truncate if exists) |
| BIND | B | socket-addr | Listen for incoming connection |
| CONNECT | CN | socket-addr | Connect to remote socket |
| READ | R | io-target | Read from file or stream |
| WRITE | W | io-target | Write to file or stream |
| SLEEP | S | milliseconds | Sleep for duration |
| HELP | H, ? | — | Show help message |

### Examples

```
# File operations
OPEN file:///tmp/input.txt
CREATE file:///tmp/output.txt
READ file:///tmp/input.txt
WRITE file:///tmp/output.txt

# Socket operations
BIND 127.0.0.1:8080
CONNECT 192.168.1.100:9000

# Stream I/O
READ stream://127.0.0.1:8080::::192.168.1.1:9000
WRITE stream://127.0.0.1:8080::::192.168.1.1:9000

# Utilities
SLEEP 1000
HELP
```

## Playbooks

Batch mode files contain one instruction per line. Comments (lines starting with `#`) and blank lines are ignored.

Example `scenario.trace2e`:
```
# Setup
OPEN file:///tmp/input.txt
CREATE file:///tmp/output.txt

# IOs
READ file:///tmp/input.txt
SLEEP 500
WRITE file:///tmp/output.txt
```

## Requirements

The `stde2e` library requires a local trace2e middleware instance running at `[::1]:50051`.

## Testing

```bash
cargo test -p trace2e_interactive
```
