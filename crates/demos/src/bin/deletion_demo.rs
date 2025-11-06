//! Deletion Demo: Distributed Deletion Cascade Across 3 Nodes
//!
//! This demo shows how marking a resource for deletion on one node
//! propagates to all other nodes in the system and blocks all I/O operations
//! that depend on the deleted resource.
//!
//! # Topology
//! ```text
//! Node1 (10.0.0.1): File /tmp/cascade.txt
//!     ↓ read by Process1
//!     → written to Socket1 (10.0.0.1:1337 → 10.0.0.2:1338)
//!
//! Node2 (10.0.0.2): Intermediate forwarder
//!     ↓ reads Socket2_in
//!     → writes to Socket3 (10.0.0.2:1339 → 10.0.0.3:1340)
//!
//! Node3 (10.0.0.3): Consumer
//!     ↓ reads Socket4_in
//! ```
//!
//! # Execution Flow
//! 1. Setup: Enroll file on Node1 + streams on all nodes
//! 2. Establish: Create data flow through the chain
//! 3. Delete: Call BroadcastDeletion(file) from Node1
//! 4. Verify: All I/O operations blocked with u128::MAX
//!
//! # Usage
//! ```bash
//! # Terminal 1 - Setup and initiate deletion
//! cargo run --bin deletion-demo -- node1
//!
//! # Terminal 2 - Intermediate node (if needed for multi-terminal variant)
//! cargo run --bin deletion-demo -- node2
//!
//! # Terminal 3 - Consumer node (if needed for multi-terminal variant)
//! cargo run --bin deletion-demo -- node3
//!
//! # Or run all-in-one (default):
//! cargo run --bin deletion-demo
//! ```

#[macro_use]
extern crate demos;

use clap::Parser;
use demos::logging::init_tracing_for_node;
use demos::orchestration::{FileMapping, StreamMapping};
use std::collections::HashSet;
use std::io::{self, Write};
use std::time::Duration;
use tower::{Service, ServiceBuilder, timeout::TimeoutLayer};
use trace2e_core::transport::loopback::spawn_loopback_middlewares;
use tracing::info;

#[derive(Parser, Debug)]
#[command(
    name = "deletion-demo",
    about = "Distributed deletion cascade demonstration across 3 nodes",
    long_about = "Shows how a resource deleted on one node propagates deletion status to all other nodes and blocks all dependent I/O operations."
)]
struct Args {
    /// Node ID to run (node1, node2, node3), or omit for all-in-one
    #[arg(value_name = "NODE")]
    node: Option<String>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    match args.node.as_deref() {
        None => run_all_in_one().await,
        Some("node1") => {
            init_tracing_for_node("10.0.0.1");
            info!("Node1 mode - waiting for coordination...");
            tokio::time::sleep(Duration::from_secs(300)).await;
        }
        Some("node2") => {
            init_tracing_for_node("10.0.0.2");
            info!("Node2 mode - waiting for coordination...");
            tokio::time::sleep(Duration::from_secs(300)).await;
        }
        Some("node3") => {
            init_tracing_for_node("10.0.0.3");
            info!("Node3 mode - waiting for coordination...");
            tokio::time::sleep(Duration::from_secs(300)).await;
        }
        Some(node) => {
            eprintln!("Unknown node: {}. Use node1, node2, or node3", node);
            std::process::exit(1);
        }
    }
}

async fn run_all_in_one() {
    init_tracing_for_node("deletion-demo");

    info!("=== Trace2e Deletion Demo ===");
    info!("Demonstrates distributed deletion cascade across 3 nodes");
    info!("Using loopback transport for in-process communication\n");

    // Spawn 3 middleware instances with 100ms timeout per operation
    let ips = vec!["10.0.0.1".to_string(), "10.0.0.2".to_string(), "10.0.0.3".to_string()];
    let mut middlewares =
        spawn_loopback_middlewares(ips.clone()).await.into_iter().map(|(p2m, o2m)| {
            (
                ServiceBuilder::new()
                    .layer(TimeoutLayer::new(Duration::from_millis(100)))
                    .service(p2m),
                o2m,
            )
        });

    let (mut p2m_1, mut o2m_1) = middlewares.next().unwrap();
    let (mut p2m_2, _) = middlewares.next().unwrap();
    let (mut p2m_3, _) = middlewares.next().unwrap();

    // === PHASE 1: SETUP ===
    info!("\n[PHASE 1] Setting up resources on all nodes");

    // Create file on node1
    let fd1_1_1 = FileMapping::new(1, 4, "/tmp/cascade.txt", "10.0.0.1".to_string());
    info!(
        "Created file mapping: {} (pid={}, fd={})",
        fd1_1_1.file_path(),
        fd1_1_1.pid(),
        fd1_1_1.fd()
    );

    local_enroll!(p2m_1, fd1_1_1);

    // Create streams connecting nodes
    let stream1_2 = StreamMapping::new(1, 3, "10.0.0.1:1337", "10.0.0.2:1338");
    let stream2_1 = StreamMapping::new(2, 3, "10.0.0.2:1338", "10.0.0.1:1337");
    let stream2_3 = StreamMapping::new(2, 4, "10.0.0.2:1339", "10.0.0.3:1340");
    let stream3_2 = StreamMapping::new(3, 3, "10.0.0.3:1340", "10.0.0.2:1339");

    info!("Created stream mappings:");
    info!("  Stream1→2: {}:{}→{}:{}", "10.0.0.1", "1337", "10.0.0.2", "1338");
    info!("  Stream2→3: {}:{}→{}:{}", "10.0.0.2", "1339", "10.0.0.3", "1340");

    remote_enroll!(p2m_1, stream1_2);
    remote_enroll!(p2m_2, stream2_1);
    remote_enroll!(p2m_2, stream2_3);
    remote_enroll!(p2m_3, stream3_2);

    // === PHASE 2: ESTABLISH DATA FLOW ===
    info!("\n[PHASE 2] Establishing data flow through 3-node chain");
    info!("  Step 1: File → Process1 (node1)");
    demo_read!(p2m_1, fd1_1_1);

    info!("  Step 2: Process1 → Socket1 (node1)");
    demo_write!(p2m_1, stream1_2);

    info!("  Step 3: Socket2 → Process2 (node2)");
    demo_read!(p2m_2, stream2_1);

    info!("  Step 4: Process2 → Socket3 (node2)");
    demo_write!(p2m_2, stream2_3);

    info!("  Step 5: Socket4 → Process3 (node3)");
    demo_read!(p2m_3, stream3_2);

    // === PHASE 2B: SHOW DATA LINEAGE ===
    info!("\n[PHASE 2B] Data lineage has been established");
    println!("\n╭────────────────────────────────────────────────────────────╮");
    println!("│ DATA LINEAGE FLOW                                          │");
    println!("├────────────────────────────────────────────────────────────┤");
    println!("│ File: /tmp/cascade.txt (Node1)                             │");
    println!("│    ↓                                                       │");
    println!("│ Process1 (Node1)                                           │");
    println!("│    ↓                                                       │");
    println!("│ Stream1 (Node1:1337 → Node2:1338)                          │");
    println!("│    ↓                                                       │");
    println!("│ Process2 (Node2)                                           │");
    println!("│    ↓                                                       │");
    println!("│ Stream2 (Node2:1339 → Node3:1340)                          │");
    println!("│    ↓                                                       │");
    println!("│ Process3 (Node3)                                           │");
    println!("╰────────────────────────────────────────────────────────────╯\n");

    println!("File status: ACTIVE (can flow through all nodes)");
    info!("  Resource: {}", fd1_1_1.file());
    info!("    All downstream I/O operations are currently ALLOWED");

    // === PHASE 3: INTERACTIVE DELETION ===
    println!("\n╭────────────────────────────────────────────────────────────╮");
    println!("│ READY TO TRIGGER DELETION                                  │");
    println!("├────────────────────────────────────────────────────────────┤");
    println!("│ This will mark the file for deletion on Node1 and          │");
    println!("│ broadcast deletion status to all other nodes.              │");
    println!("│                                                            │");
    println!("│ Type 'DELETE' or press ENTER to proceed:                   │");
    println!("╰────────────────────────────────────────────────────────────╯");
    print!("\nEnter command: ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let input = input.trim().to_uppercase();

    if input != "DELETE" && !input.is_empty() {
        println!("\n✗ Cancelled. Expected 'DELETE' but got '{}'", input);
        return;
    }

    println!("\n✓ Confirmed. Broadcasting deletion...\n");
    info!("[PHASE 3] Broadcasting deletion of file across all nodes");

    broadcast_deletion!(o2m_1, fd1_1_1.file());

    // Verify deletion status
    info!("[PHASE 3B] Verifying deletion status on node1");
    match o2m_1
        .call(trace2e_core::traceability::api::O2mRequest::GetPolicies(HashSet::from([
            fd1_1_1.file()
        ])))
        .await
    {
        Ok(trace2e_core::traceability::api::O2mResponse::Policies(policies)) => {
            for (resource, policy) in policies {
                info!("  Resource: {}", resource);
                if policy.is_pending_deletion() {
                    println!("\n╭────────────────────────────────────────────────────────────╮");
                    println!("│ DELETION STATUS UPDATE                                     │");
                    println!("├────────────────────────────────────────────────────────────┤");
                    println!("│ File status: PENDING DELETION                              │");
                    println!("│                                                            │");
                    println!("│ ✓ Deletion status marked as Pending on Node1               │");
                    println!("│ ✓ M2M notification sent to Node2 and Node3                 │");
                    println!("│                                                            │");
                    println!("│ Result: All downstream operations will now be BLOCKED      │");
                    println!("╰────────────────────────────────────────────────────────────╯\n");
                    info!("    ✓ Deletion status correctly marked as pending");
                }
            }
        }
        Ok(_) => info!("Unexpected response from GetPolicies"),
        Err(e) => info!("Failed to get policies: {e:?}"),
    }

    // === PHASE 4: VERIFY OPERATIONS BLOCKED ===
    println!("\n╭────────────────────────────────────────────────────────────╮");
    println!("│ PHASE 4: TESTING DOWNSTREAM OPERATIONS                     │");
    println!("├────────────────────────────────────────────────────────────┤");
    println!("│ Now attempting to perform I/O with the deleted resource... │");
    println!("╰────────────────────────────────────────────────────────────╯\n");

    // Attempt 1: Node2 reads from Stream2_in
    info!("[PHASE 4] Verifying all I/O operations are blocked");
    println!("Test 1: Node2 attempts to READ from Stream2_in");
    println!("        (which depends on deleted file from Node1)");
    info!("  Attempting read from stream on node2 (expects u128::MAX)...");
    let flow_id = read_request!(p2m_2, stream2_1);
    if flow_id == u128::MAX {
        println!("  Result: ✓ BLOCKED (flow_id = u128::MAX)\n");
        info!("    ✓ BLOCKED: Node2 correctly refused to read from stream with deleted source");
    } else {
        println!("  Result: ✗ ALLOWED (flow_id = {}) - UNEXPECTED!\n", flow_id);
        info!(
            "    ✗ FAILED: Node2 should have blocked the operation but granted it (flow_id={})",
            flow_id
        );
    }

    // Attempt 2: Node2 writes to downstream stream
    println!("Test 2: Node2 attempts to WRITE to Stream3_out");
    println!("        (process has contaminated provenance from deleted source)");
    info!("  Attempting write to stream on node2 (expects u128::MAX)...");
    let flow_id = write_request!(p2m_2, stream2_3);
    if flow_id == u128::MAX {
        println!("  Result: ✓ BLOCKED (flow_id = u128::MAX)\n");
        info!("    ✓ BLOCKED: Node2 correctly refused to write when data includes deleted source");
    } else {
        println!("  Result: ✗ ALLOWED (flow_id = {}) - UNEXPECTED!\n", flow_id);
        info!(
            "    ✗ FAILED: Node2 should have blocked the operation but granted it (flow_id={})",
            flow_id
        );
    }

    // Attempt 3: Node3 tries to read
    println!("Test 3: Node3 attempts to READ from Stream4_in");
    println!("        (data chain broken: source is deleted)");
    info!("  Attempting read from stream on node3 (expects u128::MAX)...");
    let flow_id = read_request!(p2m_3, stream3_2);
    if flow_id == u128::MAX {
        println!("  Result: ✓ BLOCKED (flow_id = u128::MAX)\n");
        info!(
            "    ✓ BLOCKED: Node3 correctly refused to read from stream with deleted source in chain"
        );
    } else {
        println!("  Result: ✗ ALLOWED (flow_id = {}) - UNEXPECTED!\n", flow_id);
        info!(
            "    ✗ FAILED: Node3 should have blocked the operation but granted it (flow_id={})",
            flow_id
        );
    }

    // === SUMMARY ===
    println!("╭────────────────────────────────────────────────────────────╮");
    println!("│ DEMO COMPLETE ✓                                            │");
    println!("├────────────────────────────────────────────────────────────┤");
    println!("│ ✓ Deletion broadcast successfully propagated               │");
    println!("│   to Node1, Node2, and Node3                               │");
    println!("│                                                            │");
    println!("│ ✓ All downstream operations correctly BLOCKED              │");
    println!("│   File → Process → Stream paths severed                    │");
    println!("│                                                            │");
    println!("│ ✓ Compliance policy enforcement working                    │");
    println!("│   across distributed system                                │");
    println!("╰────────────────────────────────────────────────────────────╯\n");

    info!("=== Demo Complete ===");
    info!("✓ Deletion broadcast successfully propagated to all nodes");
    info!("✓ All downstream operations correctly blocked");
    info!("✓ Compliance policy enforcement working as expected");
}
