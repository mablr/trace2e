# Trace2e E2E Demos - Quick Start

## TL;DR - Run the Demos

### Deletion Demo (Verify deletion propagates across 3 nodes)
```bash
cd /home/dan/sources/trace2e
cargo run --bin deletion-demo
```

### Consent Demo (Interactive consent enforcement)
```bash
cd /home/dan/sources/trace2e
cargo run --bin consent-demo
```

### Consent Demo (Auto-grant, no prompts)
```bash
cd /home/dan/sources/trace2e
cargo run --bin consent-demo -- --auto-grant
```

## What You'll See

### Deletion Demo Output
```
[PHASE 1] Setting up resources on all nodes
  Created file mapping: /tmp/cascade.txt
  Created stream mappings: 10.0.0.1:1337→10.0.0.2:1338, 10.0.0.2:1339→10.0.0.3:1340

[PHASE 2] Establishing data flow through 3-node chain
  File → Process1 → Socket1 → Node2 → Socket2 → Node3

[PHASE 3] Broadcasting deletion of file
  BroadcastDeletion called

[PHASE 4] Verifying all I/O operations are blocked
  ✓ BLOCKED: Node2 correctly refused to read
  ✓ BLOCKED: Node2 correctly refused to write
  ✓ BLOCKED: Node3 correctly refused to read

=== Demo Complete ===
✓ Deletion broadcast successfully propagated
✓ All downstream operations correctly blocked
✓ Compliance policy enforcement working
```

### Consent Demo Output (Interactive)
```
[PHASE 1] Setting up resources on all nodes
[PHASE 2] Enabling consent enforcement on file
[PHASE 3] Spawning consent broker
[PHASE 4] Establishing initial data flow

[CONSENT BROKER] Received consent request for destination: ...

╭────────────────────────────────────────────╮
│  CONSENT DECISION REQUIRED                 │
├────────────────────────────────────────────┤
│  Allow data flow to destination?           │
│  Enter 'y' to GRANT or 'n' to DENY        │
│  (Default: DENY after 30 seconds)        │
╰────────────────────────────────────────────╯

Your decision [y/n]: y  ← (user types y or n)

[CONSENT BROKER] Decision recorded: GRANT

=== Demo Complete ===
✓ Granted: 2
✗ Denied: 0
✓ Consent enforcement working
```

## What Each Demo Shows

### Deletion Demo ✅

**Scenario**: A file on Node1 is shared across a 3-node chain. When deleted:
1. Deletion broadcast to all nodes
2. File marked as "Pending" deletion on Node1
3. All downstream operations blocked
4. Demonstrates compliance enforcement across distributed system

**Key Property**: Deletion status propagates and blocks entire data flow chain

### Consent Demo ✅

**Scenario**: A file on Node1 requires explicit consent before data flows out:
1. Resource owner receives consent notification
2. User decides to grant or deny
3. System records decision with timestamp
4. Data flow proceeds or blocks based on decision

**Key Property**: Consent decisions routed to owner, user controls data flow

## Logging & Analysis

### View Detailed Logs
```bash
RUST_LOG=debug cargo run --bin deletion-demo 2>&1 | head -100
```

### Capture Logs to File
```bash
cargo run --bin deletion-demo > deletion_demo.log 2>&1
```

### Filter by Node (after capture)
```bash
grep "node_id: 10.0.0.1" deletion_demo.log > node1.log
grep "node_id: 10.0.0.2" deletion_demo.log > node2.log
grep "node_id: 10.0.0.3" deletion_demo.log > node3.log
```

## Testing

### Run Test Suite
```bash
bash crates/demos/test_demos.sh
```

### Expected Output
```
✓ Demos compiled successfully
✓ Deletion demo completed without errors
✓ Consent demo completed without errors
✓ Deletion demo passed (verified deletion blocks operations)
✓ Consent demo passed (verified consent enforcement)
✓ ALL TESTS PASSED
```

## Files

| File | Purpose |
|------|---------|
| `crates/demos/Cargo.toml` | Package manifest |
| `crates/demos/README.md` | Full documentation |
| `crates/demos/src/bin/deletion_demo.rs` | Deletion cascade demo (~300 lines) |
| `crates/demos/src/bin/consent_demo.rs` | Consent enforcement demo (~330 lines) |
| `crates/demos/src/macros.rs` | Helper macros |
| `crates/demos/src/logging.rs` | Structured tracing setup |
| `crates/demos/src/orchestration.rs` | Resource mapping utilities |
| `crates/demos/test_demos.sh` | Automated test suite |
| `DEMOS_SUMMARY.md` | Implementation details |

## Architecture

### 3-Node Topology (Both Demos)
```
Node1 (10.0.0.1)  →  Node2 (10.0.0.2)  →  Node3 (10.0.0.3)
   File/Resource       Intermediate       Consumer
   (Owner)             (Forwarder)        (I/O)
```

### Transport
- **In-Process Loopback**: All 3 nodes in single process
- **M2M**: Direct routing via loopback service registry

## Key Concepts

### [[memory:10390333]] - Consent Routing
Consent checks happen only on the node where the source resource resides. When remote I/O occurs, M2M requests include destination parameter so compliance service can expand provenance and check each ancestor's compliance to destination.

## Next Steps

1. **Run deletion-demo**: See how deletion propagates
2. **Run consent-demo**: Try granting/denying consent
3. **Run with logs**: Set `RUST_LOG=debug` for detailed trace
4. **Analyze logs**: Use grep to split by node_id
5. **Generate diagrams**: Post-process logs to create Mermaid flowcharts

## Troubleshooting

**Demo hangs?**
- Type 'y' or 'n' if in interactive mode
- Use `--auto-grant` flag to skip prompts
- Set timeout value (default 30s)

**No output?**
- Set `RUST_LOG=info` 
- Try: `RUST_LOG=trace2e=info cargo run --bin deletion-demo`

**Build fails?**
- Ensure workspace is recognized: `cargo build -p demos`
- Check workspace in `/home/dan/sources/trace2e/Cargo.toml`

---

**Status**: ✅ Ready to use
**Location**: `/home/dan/sources/trace2e/crates/demos/`
**Documentation**: See `crates/demos/README.md` for full details

