# TracE2E: Distributed Traceability Middleware

## Purpose

TracE2E is a **distributed traceability system** that controls and monitors I/O operations across distributed systems. It intercepts file and network I/O, maintains records of data flows (references between resources), and enforces compliance policies to ensure data only flows to authorized destinations.

## Key Capabilities

- **Data flow tracking**: Records which resources (files, streams) contributed to each data item
- **Compliance-based access control**: Grants or blocks I/O operations based on whether data is permitted to flow to the destination
- **Distributed enforcement**: Policies are checked across multiple nodes, with each node responsible for its own resources
- **Audit trail**: Complete records of all I/O operations and authorization decisions
- **Cross-node coordination**: Middleware instances communicate to verify compliance across system boundaries

## Scope

TracE2E intercepts **process I/O operations**:
- **File operations**: Reading and writing files on disk
- **Network operations**: Sending and receiving data over TCP connections

The system integrates with applications through:
- Custom I/O library wrappers (e.g., `stde2e` for standard library, patches for async libraries)
- One middleware instance per node
- Processes register resources before use and request permission for each I/O operation

These I/O operations are abstracted as data flows between resources.

## Architecture

### Communication endpoints

- **P2M**: Processes request permission for I/O operations
- **M2M**: Middleware instances coordinate to check compliance across nodes
- **O2M**: Operators manage policies and handle consent/deletion requests

### Compliance Check Design

The core of TracE2E is how it decides whether an I/O operation is allowed. For each I/O flow from a source to a destination, compliance is determined through **distributed aggregation**.

#### The Problem: Multi-Source Data

When a process wants to write to a destination file, the data it's writing may come from multiple sources:

```
Destination File: report.txt

Source Data S contains:
  ├─ Data from File A (on Node 1) - read earlier
  ├─ Data from File B (on Node 2) - read earlier  
  └─ New data created locally (on Node 1)

Question: Is it OK for this mixed data to be written to report.txt?
```

#### The Solution: Aggregate Per-Node, Check Locally

1. **Identify all sources and group by node**
   - Local node (Node 1) has: File A + locally created data
   - Remote nodes have: File B from Node 2
   
2. **Each source node checks its own resources compliance**
   - Node 1 checks: Can File A → report.txt? (using its policies & consent)
   - Node 1 asks Node 2: Can File B → report.txt?
   - Node 2 checks locally: Can File B → report.txt?

3. **Grant if all agree**
   - If Node 1 says YES and Node 2 says YES → **ALLOW** the write
   - If either says NO → **BLOCK** the write

#### Key Properties

- **Decentralized decisions**: Node X owns the decision for data originating from Node X
- **Aggregation**: All source nodes must approve before the flow is permitted
- **Auditability**: Each node records its own compliance decisions
- **Scalability**: Can handle arbitrary numbers of source nodes

### System Components

**Provenance Tracking**
- Records which sources (files, streams, processes) contributed to each data item
- Builds a lineage graph showing data flow through the system

**Compliance Engine**
- For each source→destination flow, determines if the flow is allowed
- Local evaluation: checks policies and organizational rules on the source node
- Remote requests: asks other nodes to check their resources via M2M communication
- Aggregates results: grants access only if all sources approve

**Consent Management**
- Tracks explicit user/organization approval for specific data flows
- Integrated into compliance decisions (can block even if policies allow)

**Flow Coordination**
- Sequences I/O operations to maintain consistency
- Reserves resources to prevent conflicts and race conditions

### How It Works Together

When a process performs an I/O operation:

1. **Process requests permission** through P2M (e.g., "Can I write this data to file X?")
2. **Local middleware gathers provenance** - what are the sources of this data?
3. **Local middleware checks compliance** - are all sources allowed to flow to destination X?
4. **For remote sources, M2M requests** are sent to other nodes to verify their compliance
5. **Remote nodes check locally** - can their resources flow to destination X?
6. **Results aggregate** - if all sources approve, the operation proceeds
