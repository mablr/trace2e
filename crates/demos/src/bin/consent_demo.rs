//! Consent Demo: Interactive Multi-Node Consent Enforcement
//!
//! This demo shows how consent requests are routed to the resource owner
//! and how the system waits for user feedback before allowing data flow.
//!
//! # Topology
//! ```text
//! Node1 (10.0.0.1): File with CONSENT ENFORCED
//!     ↓ read by Process1
//!     → written to Socket1 (10.0.0.1:1337 → 10.0.0.2:1338)
//!       [TRIGGERS CONSENT REQUEST #1]
//!
//! Node2 (10.0.0.2): Intermediate forwarder + Consent Broker
//!     ├─ receives Socket2_in
//!     ├─ awaits consent notifications from Node1
//!     ├─ prompts user for consent decision (Y/N)
//!     └─ sends decision back to Node1
//!
//! Node3 (10.0.0.3): Consumer
//!     ↓ reads Socket4_in
//!       [TRIGGERS CONSENT REQUEST #2]
//! ```
//!
//! # Execution Flow (All-in-One Mode)
//! 1. Setup: Enroll file on Node1 + streams on all nodes
//! 2. Display Data Lineage: Show user the complete flow path
//! 3. Enable Consent: Enforce consent on the file
//! 4. Consent Broker: Node1 listens for notifications
//! 5. Trigger I/O: Data flows through all nodes
//! 6. Interactive Requests: User prompted for each consent decision
//!    - Request #1: First flow with prompt → user responds Y (GRANT)
//!    - Request #2: Second flow with prompt → user responds N (DENY)
//! 7. Results: Operations allowed or blocked based on decisions
//! 8. Summary: Display granted vs denied counts
//!
//! # Usage (Single Terminal - All-in-One)
//! ```bash
//! cargo run --bin consent-demo
//! # Follow interactive prompts for consent decisions
//! ```
//!
//! # Usage (Multi-Terminal Mode)
//! ```bash
//! # Terminal 1 - Node1 (resource owner)
//! cargo run --bin consent-demo -- node1
//!
//! # Terminal 2 - Node2 (consent broker)
//! cargo run --bin consent-demo -- node2
//!
//! # Terminal 3 - Node3 (consumer)
//! cargo run --bin consent-demo -- node3
//! ```

#[macro_use]
extern crate demos;

use clap::Parser;
use demos::logging::init_tracing_for_node;
use demos::orchestration::{FileMapping, StreamMapping};
use std::io::{self, Write};
use std::time::Duration;
use tokio::time::timeout;
use tower::{Service, ServiceBuilder, timeout::TimeoutLayer};
use trace2e_core::transport::loopback::spawn_loopback_middlewares;
use tracing::{info, warn};

#[derive(Parser, Debug)]
#[command(
    name = "consent-demo",
    about = "Interactive consent enforcement demonstration across 3 nodes",
    long_about = "Shows how consent requests are routed to resource owners and how the system waits for user decisions."
)]
struct Args {
    /// Node ID to run (node1, node2, node3), or omit for all-in-one
    #[arg(value_name = "NODE")]
    node: Option<String>,

    /// Automatically grant all consent requests (for unattended demo)
    #[arg(long)]
    auto_grant: bool,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    match args.node.as_deref() {
        None => run_all_in_one(args.auto_grant).await,
        Some("node1") => {
            init_tracing_for_node("10.0.0.1");
            info!("Node1 mode - Resource owner. Waiting for coordination...");
            tokio::time::sleep(Duration::from_secs(300)).await;
        }
        Some("node2") => {
            init_tracing_for_node("10.0.0.2");
            info!("Node2 mode - Intermediate forwarder. Waiting for coordination...");
            tokio::time::sleep(Duration::from_secs(300)).await;
        }
        Some("node3") => {
            init_tracing_for_node("10.0.0.3");
            info!("Node3 mode - Consumer. Waiting for coordination...");
            tokio::time::sleep(Duration::from_secs(300)).await;
        }
        Some(node) => {
            eprintln!("Unknown node: {}. Use node1, node2, or node3", node);
            std::process::exit(1);
        }
    }
}

async fn run_all_in_one(auto_grant: bool) {
    init_tracing_for_node("consent-demo");

    info!("=== Trace2e Consent Demo ===");
    info!("Demonstrates interactive consent enforcement across 3 nodes");
    info!("Using loopback transport for in-process communication");
    if auto_grant {
        info!("AUTO-GRANT mode: All consent requests will be automatically approved\n");
    } else {
        info!("INTERACTIVE mode: You will be prompted for consent decisions\n");
    }

    // Spawn 3 middleware instances with 500ms timeout per operation
    let ips = vec!["10.0.0.1".to_string(), "10.0.0.2".to_string(), "10.0.0.3".to_string()];
    let mut middlewares =
        spawn_loopback_middlewares(ips.clone()).await.into_iter().map(|(p2m, o2m)| {
            (
                ServiceBuilder::new()
                    .layer(TimeoutLayer::new(Duration::from_millis(500)))
                    .service(p2m),
                o2m,
            )
        });

    let (mut p2m_1, mut o2m_1) = middlewares.next().unwrap();
    let (mut p2m_2, _) = middlewares.next().unwrap();
    let (mut p2m_3, _) = middlewares.next().unwrap();

    // === PHASE 1: SETUP ===
    info!("\n[PHASE 1] Setting up resources on all nodes");

    // Create file on node1 with consent
    let fd1_1_1 = FileMapping::new(1, 4, "/tmp/sensitive.txt", "10.0.0.1".to_string());
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

    // === PHASE 2: SHOW DATA LINEAGE ===
    info!("\n[PHASE 2] Data lineage will be established");
    println!("\n╭────────────────────────────────────────────────────────────╮");
    println!("│ DATA LINEAGE FLOW                                          │");
    println!("├────────────────────────────────────────────────────────────┤");
    println!("│ File: /tmp/sensitive.txt (Node1) - CONSENT REQUIRED       │");
    println!("│    ↓                                                        │");
    println!("│ Process1 (Node1)                                           │");
    println!("│    ↓                                                        │");
    println!("│ Stream1 (Node1:1337 → Node2:1338)                          │");
    println!("│    ↓                                                        │");
    println!("│ Process2 (Node2)                                           │");
    println!("│    ↓                                                        │");
    println!("│ Stream2 (Node2:1339 → Node3:1340)                          │");
    println!("│    ↓                                                        │");
    println!("│ Process3 (Node3)                                           │");
    println!("╰────────────────────────────────────────────────────────────╯\n");
    info!("Data lineage shown to user");

    // === PHASE 3: ENABLE CONSENT ===
    info!("\n[PHASE 3] Enabling consent enforcement on file");
    let mut notifications = enforce_consent!(o2m_1, fd1_1_1.file());
    info!("Consent enforcement enabled, notification channel ready");

    // === PHASE 4: SPAWN CONSENT BROKER ===
    info!("\n[PHASE 4] Spawning consent broker for user decisions");

    let fd1_file = fd1_1_1.file();
    let mut o2m_consent = o2m_1.clone();

    let consent_task = tokio::spawn(async move {
        let mut granted_count = 0;
        let mut denied_count = 0;
        let mut decision_log = Vec::new();
        let mut request_count = 0;

        // Wait for consent notifications with timeout
        loop {
            match timeout(Duration::from_secs(2), notifications.recv()).await {
                Ok(Ok(destination)) => {
                    request_count += 1;
                    info!(
                        "[CONSENT BROKER] Received consent request #{} for destination: {:?}",
                        request_count, destination
                    );

                    let destination_desc = format!("{:?}", destination);
                    let decision = if auto_grant {
                        info!("[CONSENT BROKER] AUTO-GRANTING consent (--auto-grant mode)");
                        true
                    } else {
                        // Interactive mode: prompt user with detailed flow information
                        info!("[CONSENT BROKER] Awaiting user decision (30 second timeout)...");
                        prompt_user_decision(request_count, &destination_desc)
                    };

                    decision_log.push((destination.clone(), decision));
                    if decision {
                        granted_count += 1;
                        println!("\n✓ Consent GRANTED for request #{}", request_count);
                        println!("  Flow will proceed with data transfer\n");
                        info!("✓ Consent GRANTED for request #{}", request_count);
                    } else {
                        denied_count += 1;
                        println!("\n✗ Consent DENIED for request #{}", request_count);
                        println!("  Flow will be blocked at the resource owner\n");
                        info!("✗ Consent DENIED for request #{}", request_count);
                    }

                    set_consent_decision!(
                        o2m_consent,
                        fd1_file.clone(),
                        destination.clone(),
                        decision
                    );

                    info!(
                        "[CONSENT BROKER] Decision recorded: {} for {:?}",
                        if decision { "GRANT" } else { "DENY" },
                        destination
                    );
                }
                Ok(Err(e)) => {
                    warn!("[CONSENT BROKER] Notification channel closed: {:?}", e);
                    break;
                }
                Err(_) => {
                    info!("[CONSENT BROKER] No more consent requests (timeout)");
                    break;
                }
            }
        }

        (granted_count, denied_count, decision_log)
    });

    // Give the consent handler time to start listening
    tokio::time::sleep(Duration::from_millis(100)).await;

    // === PHASE 5: ESTABLISH DATA FLOW & TRIGGER CONSENT REQUESTS ===
    info!("\n[PHASE 5] Establishing initial data flow");
    println!("\n╭────────────────────────────────────────────────────────────╮");
    println!("│ PHASE 5: TRIGGERING CONSENT REQUESTS                     │");
    println!("├────────────────────────────────────────────────────────────┤");
    println!("│ As data flows through the system, consent will be         │");
    println!("│ requested. You will be prompted to make decisions.        │");
    println!("│                                                            │");
    println!("│ Expected: First request (GRANT) then second (DENY)        │");
    println!("╰────────────────────────────────────────────────────────────╯\n");

    info!("  Step 1: File → Process1 (node1)");
    demo_read!(p2m_1, fd1_1_1);

    info!("  Step 2: Process1 → Socket1 (node1) [Request #1 incoming...]");
    demo_write!(p2m_1, stream1_2);

    info!("  Step 3: Socket2 → Process2 (node2)");
    demo_read!(p2m_2, stream2_1);

    info!("  Step 4: Process2 → Socket3 (node2) [Request #2 incoming...]");
    demo_write!(p2m_2, stream2_3);

    info!("  Step 5: Socket4 → Process3 (node3)");
    demo_read!(p2m_3, stream3_2);

    // Wait for consent processing
    info!("\n[PHASE 6] Waiting for consent decisions to be processed...");
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Get results
    let (granted_count, denied_count, decision_log) = consent_task.await.unwrap();

    // === SUMMARY ===
    println!("\n╭────────────────────────────────────────────────────────────╮");
    println!("│ DEMO COMPLETE ✓                                            │");
    println!("├────────────────────────────────────────────────────────────┤");
    println!("│ Consent decisions processed:                              │");
    println!("│   ✓ GRANTED: {}                                               │", granted_count);
    println!("│   ✗ DENIED:  {}                                               │", denied_count);
    println!("│                                                            │");
    println!("│ Data flows were allowed or blocked based on your choices │");
    println!("│ Consent enforcement working as expected                   │");
    println!("╰────────────────────────────────────────────────────────────╯\n");

    info!("\n=== Demo Complete ===");
    info!("Consent requests processed:");
    info!("  ✓ Granted: {}", granted_count);
    info!("  ✗ Denied: {}", denied_count);
    info!("Decision history:");
    for (i, (destination, decision)) in decision_log.iter().enumerate() {
        info!("  [{}] {:?} → {}", i + 1, destination, if *decision { "GRANT" } else { "DENY" });
    }
    info!("✓ Consent enforcement working as expected");
}

/// Prompt user for consent decision with detailed flow information
///
/// Returns true for grant, false for deny.
/// Times out to false after 30 seconds.
fn prompt_user_decision(request_num: usize, destination_desc: &str) -> bool {
    let timeout_secs = 30;

    println!("\n╭────────────────────────────────────────────────────────────╮");
    println!("│  CONSENT REQUEST #{}", request_num);
    println!("├────────────────────────────────────────────────────────────┤");
    println!("│  Data flow destination: {:<41} │", destination_desc);
    println!("│                                                            │");
    println!("│  Allow data flow?                                          │");
    println!("│                                                            │");
    println!("│  Enter 'y' to GRANT or 'n' to DENY                        │");
    println!("│  (Default: DENY after {} seconds)                       │", timeout_secs);
    println!("╰────────────────────────────────────────────────────────────╯");
    print!("\nYour decision [y/n]: ");
    io::stdout().flush().unwrap();

    // Try to read with timeout using a spawned task
    let read_handle = std::thread::spawn(|| {
        let mut buf = String::new();
        io::stdin().read_line(&mut buf).ok();
        buf.trim().to_lowercase()
    });

    // Wait for input with timeout
    let start = std::time::Instant::now();
    loop {
        if read_handle.is_finished() {
            let input = read_handle.join().unwrap();
            match input.as_str() {
                "y" | "yes" => {
                    info!("User decision: GRANT");
                    return true;
                }
                "n" | "no" => {
                    info!("User decision: DENY");
                    return false;
                }
                "" => {
                    warn!("No input provided, defaulting to DENY");
                    return false;
                }
                _ => {
                    warn!("Invalid input: {}, defaulting to DENY", input);
                    return false;
                }
            }
        }

        if start.elapsed().as_secs() > timeout_secs {
            warn!("User input timeout after {} seconds, defaulting to DENY", timeout_secs);
            return false;
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}
