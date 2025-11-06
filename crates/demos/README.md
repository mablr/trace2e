# Trace2e E2E Demonstrations

This crate contains two comprehensive end-to-end demonstrations of Trace2e's core compliance properties:

1. **Deletion Demo** - Distributed deletion cascade across 3 nodes
2. **Consent Demo** - Interactive consent enforcement with user feedback

Both demos use the **loopback transport** layer for in-process communication between 3 middleware instances representing separate nodes.

## Architecture

### Topology (Both Demos)

```
Node1 (10.0.0.1)           Node2 (10.0.0.2)           Node3 (10.0.0.3)
┌──────────────┐           ┌──────────────┐           ┌──────────────┐
│              │           │              │           │              │
│   File       │──write──> │  Stream2_in  │──write──> │  Stream4_in  │
│              │           │              │           │              │
│   Socket1    │<──read─── │  Stream2_out │<──read─── │   Process3   │
│              │           │              │           │              │
└──────────────┘           └──────────────┘           └──────────────┘
       ^                           ^                           ^
       │                           │                           │
    Process1                    Process2                   (Consumer)
```

### Transport

- **In-Process Loopback**: All 3 nodes run in a single process
- **M2M Communication**: Direct routing via broadcast/targeted calls
- **Resource Enrollment**: Files, streams, and processes enrolled programmatically
- **Data Flow**: Sequential read/write operations across the chain

## Deletion Demo

### Purpose

Demonstrates that deleting a resource on one node propagates deletion status to all other nodes and blocks all I/O operations that depend on the deleted resource.

### Execution Flow

1. **Setup**: Enroll file on Node1 and streams on all nodes
2. **Establish**: Create data flow: File → Process1 → Stream1 → Node2 → Stream2 → Node3
3. **Delete**: Call `BroadcastDeletion(file)` from Node1
4. **Verify**: All downstream I/O operations blocked (return `u128::MAX`)

### Key Properties Demonstrated

- ✅ Resource deletion broadcasts to all nodes
- ✅ Deletion status correctly marked as "Pending"
- ✅ All downstream operations refuse further I/O
- ✅ Compliance enforcement prevents data flow from deleted sources

### Running

```bash
# Build
cargo build -p demos

# Run all-in-one demo
cargo run --bin deletion-demo

# Expected output shows:
# - File enrollment on Node1
# - Stream enrollment on Nodes 1, 2, 3
# - Initial data flow establishment
# - BroadcastDeletion call
# - Verification that operations are blocked
```

### Example Output

```
[PHASE 1] Setting up resources on all nodes
  Created file mapping: /tmp/cascade.txt (pid=1, fd=4)
  Created stream mappings:
    Stream1→2: 10.0.0.1:1337→10.0.0.2:1338
    Stream2→3: 10.0.0.2:1339→10.0.0.3:1340

[PHASE 2] Establishing data flow through 3-node chain
  Step 1: File → Process1 (node1)
  Step 2: Process1 → Socket1 (node1)
  Step 3: Socket2 → Process2 (node2)
  Step 4: Process2 → Socket3 (node2)
  Step 5: Socket4 → Process3 (node3)

[PHASE 3] Broadcasting deletion of file across all nodes
  Calling BroadcastDeletion(file) from node1

[PHASE 4] Verifying all I/O operations are blocked
  Attempting read from stream on node2 (expects u128::MAX)...
    ✓ BLOCKED: Node2 correctly refused to read from stream with deleted source
  Attempting write to stream on node2 (expects u128::MAX)...
    ✓ BLOCKED: Node2 correctly refused to write when data includes deleted source
  Attempting read from stream on node3 (expects u128::MAX)...
    ✓ BLOCKED: Node3 correctly refused to read from stream with deleted source in chain

=== Demo Complete ===
✓ Deletion broadcast successfully propagated to all nodes
✓ All downstream operations correctly blocked
✓ Compliance policy enforcement working as expected
```

## Consent Demo

### Purpose

Demonstrates how consent requests are routed to the resource owner and how the system waits for user feedback before allowing data flow.

### Key Concept: Consent Routing

- Resource owner (Node1) holds the file with consent enforcement enabled
- When remote I/O operations occur, consent requests are sent to owner
- Owner receives notification and must explicitly grant/deny consent
- System waits for response (30 second timeout, auto-deny on no response)

### Execution Flow

1. **Setup**: Enroll file on Node1 and streams on all nodes
2. **Enable Consent**: Call `EnforceConsent(file)` on Node1
3. **Broker**: Spawn task to listen for consent notifications
4. **Trigger I/O**: Perform read/write operations
5. **Interactive**: System prompts user for Y/N consent decision
6. **Response**: User input → `SetConsentDecision` sent back
7. **Complete**: I/O completes or fails based on decision

### Key Properties Demonstrated

- ✅ Consent notifications routed to resource owner
- ✅ System waits for user decision (with timeout)
- ✅ User can grant or deny consent
- ✅ Decisions recorded with timestamp
- ✅ Multiple consent flows handled sequentially

### Running

```bash
# Run with interactive prompts
cargo run --bin consent-demo

# Run in auto-grant mode (no prompts)
cargo run --bin consent-demo -- --auto-grant

# Run single node (for multi-terminal coordination)
cargo run --bin consent-demo -- node1  # In terminal 1
cargo run --bin consent-demo -- node2  # In terminal 2
cargo run --bin consent-demo -- node3  # In terminal 3
```

### Interactive Prompt Example

```
╭────────────────────────────────────────────╮
│  CONSENT DECISION REQUIRED                 │
├────────────────────────────────────────────┤
│  Allow data flow to destination?           │
│                                            │
│  Enter 'y' to GRANT or 'n' to DENY        │
│  (Default: DENY after 30 seconds)        │
╰────────────────────────────────────────────╯

Your decision [y/n]: _
```

### Example Output (Auto-Grant Mode)

```
[PHASE 1] Setting up resources on all nodes
  Created file mapping: /tmp/sensitive.txt (pid=1, fd=4)
  Created stream mappings:
    Stream1→2: 10.0.0.1:1337→10.0.0.2:1338
    Stream2→3: 10.0.0.2:1339→10.0.0.3:1340

[PHASE 2] Enabling consent enforcement on file
  Consent enforcement enabled, notification channel ready

[PHASE 3] Spawning consent broker for user decisions
  AUTO-GRANT mode: All consent requests will be automatically approved

[PHASE 4] Establishing initial data flow
  Step 1: File → Process1 (node1)
  Step 2: Process1 → Socket1 (node1)
    [Triggers consent request for flow to node2...]
  [CONSENT BROKER] Received consent request for destination: ...
  [CONSENT BROKER] AUTO-GRANTING consent (--auto-grant mode)
  [CONSENT BROKER] Decision recorded: GRANT
  ...

=== Demo Complete ===
Consent requests processed:
  ✓ Granted: 2
  ✗ Denied: 0
Decision history:
  [1] {:?} → GRANT
  [2] {:?} → GRANT
✓ Consent enforcement working as expected
```

## Logging

### Structured Output

Both demos use structured tracing with:
- **Timestamp**: ISO 8601 format with millisecond precision
- **Level**: INFO, DEBUG, WARN, ERROR
- **Module**: Component (demos::logging, trace2e_core::*, etc.)
- **Fields**: Context-specific data (node_id, resource, policy, etc.)

### Filtering

Set `RUST_LOG` environment variable:

```bash
# All demo logs
RUST_LOG=demos=debug cargo run --bin deletion-demo

# Specific components
RUST_LOG=trace2e_core=debug,demos=info cargo run --bin deletion-demo

# All logs
RUST_LOG=debug cargo run --bin deletion-demo
```

### Log Splitting (Future)

Once logs are captured to files, they can be split by node_id:

```bash
# Capture deletion demo logs
cargo run --bin deletion-demo > deletion_demo_full.log 2>&1

# Split by node_id for Mermaid diagram generation
grep "node_id: 10.0.0.1" deletion_demo_full.log > deletion_demo_node1.log
grep "node_id: 10.0.0.2" deletion_demo_full.log > deletion_demo_node2.log
grep "node_id: 10.0.0.3" deletion_demo_full.log > deletion_demo_node3.log
```

## Implementation Details

### Shared Utilities

Located in `src/`:

- **`logging.rs`**: Structured tracing setup with `init_tracing_for_node()`
- **`orchestration.rs`**: Resource mapping structures (FileMapping, StreamMapping)
- **`macros.rs`**: Helper macros for common operations:
  - `local_enroll!`, `remote_enroll!`
  - `read_request!`, `write_request!`, `io_report!`
  - `demo_read!`, `demo_write!` (combined request + report)
  - `broadcast_deletion!`, `enforce_consent!`, `set_consent_decision!`

### Binaries

- **`bin/deletion_demo.rs`**: Deletion cascade demonstration (~300 lines)
- **`bin/consent_demo.rs`**: Consent enforcement with interactive CLI (~330 lines)

### No Changes to trace2e_core

✅ All demos use only public APIs from `trace2e_core`:
- `P2mRequest`, `P2mResponse`
- `O2mRequest`, `O2mResponse`
- `M2mRequest`, `M2mResponse`
- `spawn_loopback_middlewares()`
- `FileMapping`, `StreamMapping` (test fixtures)

## Future Enhancements

1. **Multi-Terminal Mode**: Run each node in separate terminal/process
2. **Mermaid Diagram Generation**: Convert logs to visual flowcharts
3. **Timing Analysis**: Measure end-to-end latency of operations
4. **Metrics Export**: Prometheus-compatible metrics output
5. **Compliance Report**: Automated compliance verification summary

## Troubleshooting

### Demo Hangs

**Problem**: Demo appears to hang or takes too long to complete

**Solution**: 
- Check that loopback transport is properly initialized
- Verify timeout values are not too long
- Enable debug logging: `RUST_LOG=debug`

### Consent Never Granted

**Problem**: Interactive consent demo waits forever for user input

**Solution**:
- Type 'y' or 'n' followed by Enter
- Or use `--auto-grant` flag for unattended mode
- Default timeout is 30 seconds (auto-deny)

### No Output

**Problem**: Demo runs but produces no log output

**Solution**:
- Set `RUST_LOG=info` or higher
- Try: `RUST_LOG=trace2e=info cargo run --bin deletion-demo`
- Add `tracing_subscriber::fmt()` debugging

## Architecture Notes

### Loopback Transport Benefits

1. ✅ All 3 nodes in single process = easy debugging
2. ✅ No network overhead or delays
3. ✅ Deterministic M2M communication
4. ✅ Easy to add breakpoints and inspect state

### Limitations

1. ⚠️ Cannot simulate network failures
2. ⚠️ Cannot test gRPC serialization
3. ⚠️ Logs from all nodes mixed (need parsing to split)
4. ⚠️ Single-process resource constraints

### Design Choices

- **No modifications to trace2e_core**: Demonstrates external instrumentation capability
- **Test fixture reuse**: Leverages existing `FileMapping`, `StreamMapping` patterns
- **Macro-based helpers**: Reduces boilerplate in demo code
- **Structured tracing**: Enables future log analysis and visualization

## References

- See `crates/trace2e_core/src/tests/` for unit test patterns
- See `crates/demos/src/macros.rs` for available helper macros
- Consult `trace2e_core` docs for API details


