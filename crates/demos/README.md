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

### User Experience

The demo is **fully interactive**:

1. **Phase 2B**: Visual display of data lineage showing the complete 3-node flow
2. **Phase 3**: User prompted to type `DELETE` to trigger deletion (intentional action)
3. **Phase 3B**: Real-time status update showing deletion was broadcast and marked as "PENDING"
4. **Phase 4**: Sequential tests showing each downstream operation being blocked with clear reasons
5. **Summary**: Final verification that compliance enforcement works across all nodes

Example output:
```
╭────────────────────────────────────────────────────────────╮
│ DATA LINEAGE FLOW                                          │
├────────────────────────────────────────────────────────────┤
│ File: /tmp/cascade.txt (Node1)                             │
│    ↓ Process1 → Stream1 → Node2 → Stream2 → Node3          │
│ Process3 (Node3)                                           │
╰────────────────────────────────────────────────────────────╯
File status: ACTIVE (can flow through all nodes)

[User types: DELETE]

╭────────────────────────────────────────────────────────────╮
│ DELETION STATUS UPDATE                                       │
│ File status: PENDING DELETION                              │
│ ✓ Deletion broadcasted to all nodes                        │
│ Result: All downstream operations will now be BLOCKED      │
╰────────────────────────────────────────────────────────────╯

Test 1: Node2 READ → Result: ✓ BLOCKED
Test 2: Node2 WRITE → Result: ✓ BLOCKED  
Test 3: Node3 READ → Result: ✓ BLOCKED
```

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

Demonstrates how consent requests are routed to the resource owner and how the system waits for user feedback before allowing data flow. Features **fully interactive UI** where users control data flow decisions.

### User Experience

The demo is **fully interactive** and walks through the complete consent lifecycle:

1. **Phase 2**: Visual display of data lineage showing the complete 3-node flow
2. **Phase 5**: Trigger data flow operations
3. **Consent Request #1**: User prompted to GRANT or DENY the first data flow
   - Default: User responds **Y** (GRANT) → flow proceeds
4. **Consent Request #2**: User prompted again for second data flow
   - Default: User responds **N** (DENY) → flow is blocked
5. **Summary**: Display of granted vs denied counts

Example output:
```
╭────────────────────────────────────────────────────────────╮
│ DATA LINEAGE FLOW                                          │
├────────────────────────────────────────────────────────────┤
│ File: /tmp/sensitive.txt (Node1) - CONSENT REQUIRED       │
│    ↓                                                        │
│ Process1 (Node1)                                           │
│    ↓                                                        │
│ Stream1 (Node1:1337 → Node2:1338)                          │
│    ↓                                                        │
│ Process2 (Node2)                                           │
│    ↓                                                        │
│ Stream2 (Node2:1339 → Node3:1340)                          │
│    ↓                                                        │
│ Process3 (Node3)                                           │
╰────────────────────────────────────────────────────────────╯

╭────────────────────────────────────────────────────────────╮
│ PHASE 5: TRIGGERING CONSENT REQUESTS                      │
├────────────────────────────────────────────────────────────┤
│ As data flows through the system, consent will be         │
│ requested. You will be prompted to make decisions.        │
│                                                            │
│ Expected: First request (GRANT) then second (DENY)        │
╰────────────────────────────────────────────────────────────╯

[Request #1 triggers...]

✓ Consent GRANTED for request #1
  Flow will proceed with data transfer

[Request #2 triggers...]

✗ Consent DENIED for request #2
  Flow will be blocked at the resource owner

╭────────────────────────────────────────────────────────────╮
│ DEMO COMPLETE ✓                                            │
├────────────────────────────────────────────────────────────┤
│ Consent decisions processed:                              │
│   ✓ GRANTED: 1                                               │
│   ✗ DENIED:  1                                               │
│                                                            │
│ Data flows were allowed or blocked based on your choices │
│ Consent enforcement working as expected                   │
╰────────────────────────────────────────────────────────────╯
```

### Key Concept: Consent Routing

- Resource owner (Node1) holds the file with consent enforcement enabled
- When remote I/O operations occur, consent requests are sent to owner
- Owner receives notification and must explicitly grant/deny consent
- System waits for response (30 second timeout, auto-deny on no response)

### Execution Flow

1. **Phase 1**: Enroll file on Node1 and streams on all nodes
2. **Phase 2**: Enable consent enforcement on file
3. **Phase 3**: Spawn consent broker to listen for notifications
4. **Phase 4**: Trigger I/O operations across all nodes
5. **Phase 5**: For each consent notification:
   - Display detailed prompt with destination info
   - Wait for user Y/N response (or auto-grant)
6. **Phase 6**: Wait for decision to grant or deny flow
7. **Summary**: Display final count of granted vs denied

### Key Properties Demonstrated

- ✅ Consent notifications routed to resource owner
- ✅ System waits for user decision (with timeout)
- ✅ User can grant or deny consent with visual feedback
- ✅ Decisions instantly reflected in data flow outcomes
- ✅ Multiple consent flows handled with clear UI per request

### Running

```bash
# Run with interactive prompts (user types y/n)
cargo run --bin consent-demo

# Run in auto-grant mode (all requests approved)
cargo run --bin consent-demo -- --auto-grant

# Run single node (for multi-terminal coordination)
cargo run --bin consent-demo -- node1  # In terminal 1
cargo run --bin consent-demo -- node2  # In terminal 2
cargo run --bin consent-demo -- node3  # In terminal 3
```

### Interactive Prompt Example

```
╭────────────────────────────────────────────────────────────╮
│  CONSENT REQUEST #1                                        │
├────────────────────────────────────────────────────────────┤
│  Data flow destination: Destination { ... }                │
│                                                            │
│  Allow data flow?                                          │
│                                                            │
│  Enter 'y' to GRANT or 'n' to DENY                         │
│  (Default: DENY after 30 seconds)                          │
╰────────────────────────────────────────────────────────────╯

Your decision [y/n]: y

✓ Consent GRANTED for request #1
  Flow is done...
```

### Example Output (Auto-Grant Mode)

```
[PHASE 1] Setting up resources on all nodes
  Created file mapping: /tmp/sensitive.txt (pid=1, fd=4)
  Created stream mappings:
    Stream1→2: 10.0.0.1:1337→10.0.0.2:1338
    Stream2→3: 10.0.0.2:1339→10.0.0.3:1340

[PHASE 2] Data lineage will be established
[Shows visual ASCII representation]

[PHASE 3] Enabling consent enforcement on file
  Consent enforcement enabled, notification channel ready

[PHASE 4] Spawning consent broker for user decisions

[PHASE 5] Establishing initial data flow
  Step 1: File → Process1 (node1)
  Step 2: Process1 → Socket1 (node1) [Request #1 incoming...]
  [CONSENT BROKER] Received consent request #1
  [CONSENT BROKER] AUTO-GRANTING consent (--auto-grant mode)
  ✓ Consent GRANTED for request #1
  ...

=== Demo Complete ===
Consent requests processed:
  ✓ Granted: 5
  ✗ Denied: 0
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

### Log Splitting

Once logs are captured to files, they can be split by node_id:

```bash
# Capture deletion demo logs
cargo run --bin deletion-demo > deletion_demo_full.log 2>&1

# Split by node_id for Mermaid diagram generation
grep "node_id: 10.0.0.1" deletion_demo_full.log > deletion_demo_node1.log
grep "node_id: 10.0.0.2" deletion_demo_full.log > deletion_demo_node2.log
grep "node_id: 10.0.0.3" deletion_demo_full.log > deletion_demo_node3.log
```


