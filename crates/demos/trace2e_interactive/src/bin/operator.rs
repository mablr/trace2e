//! CLI tool for interacting with the trace2e O2M (Operator-to-Middleware) API
//!
//! This tool allows operators to manage compliance policies, consent, and provenance queries.
//!
//! # Examples
//!
//! Get policies for resources:
//! ```bash
//! trace2e-operator get-policies "file:///path/to/file" "stream://127.0.0.1:8080::192.168.1.1:9000"
//! ```
//!
//! Set policy:
//! ```bash
//! trace2e-operator set-policy "file:///path/to/file" --confidentiality 1 --integrity 1
//! ```
//!
//! Enforce consent and stream notifications:
//! ```bash
//! trace2e-operator enforce-consent "file:///data.txt"
//! ```
//!
//! Set consent decision (in another terminal):
//! ```bash
//! trace2e-operator set-consent-decision \
//!   --source "file:///data.txt" \
//!   --destination "node1" \
//!   --grant
//! ```

use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use std::convert::TryFrom;
use tokio_stream::StreamExt;
use trace2e_client::{o2m, primitives};
use trace2e_core::traceability::infrastructure::naming;
use trace2e_core::traceability::services::consent::Destination;
use trace2e_core::transport::grpc::proto::messages::ConsentNotification;

/// Parse resource string into naming::Resource
fn parse_resource(s: &str) -> Result<naming::Resource> {
    naming::Resource::try_from(s).map_err(|e| anyhow!("Failed to parse resource: {}", e))
}

#[derive(Parser)]
#[command(name = "trace2e-operator")]
#[command(about = "Operator tool for trace2e middleware compliance and provenance management")]
#[command(version)]
struct Cli {
    /// Middleware host address
    #[arg(long, global = true, env = "TRACE2E_HOST", default_value = "127.0.0.1")]
    host: String,

    /// Middleware port
    #[arg(long, global = true, env = "TRACE2E_PORT", default_value = "50051")]
    port: u16,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Retrieve compliance policies for resources
    GetPolicies {
        /// Resource URLs to query (file:///path or stream://local::peer)
        #[arg(required = true)]
        resources: Vec<String>,
    },

    /// Set complete compliance policy for a resource
    SetPolicy {
        /// Target resource
        resource: String,

        /// Confidentiality level (0=PUBLIC, 1=SECRET)
        #[arg(long)]
        confidentiality: Option<i32>,

        /// Minimum integrity level (higher = stricter)
        #[arg(long)]
        integrity: Option<u32>,
    },

    /// Set confidentiality requirement for a resource
    SetConfidentiality {
        /// Target resource
        resource: String,

        /// Confidentiality level (0=PUBLIC, 1=SECRET)
        #[arg(long)]
        level: i32,
    },

    /// Set integrity level requirement for a resource
    SetIntegrity {
        /// Target resource
        resource: String,

        /// Integrity level
        #[arg(long)]
        level: u32,
    },

    /// Mark a resource as deleted
    SetDeleted {
        /// Resource to mark as deleted
        resource: String,
    },

    /// Broadcast deletion to all middleware instances
    BroadcastDeletion {
        /// Resource that was deleted
        resource: String,
    },

    /// Enforce consent requirement for a resource (streams notifications)
    EnforceConsent {
        /// Resource to enforce consent for
        resource: String,
    },

    /// Set consent decision for a data flow
    SetConsentDecision {
        /// Source resource
        #[arg(long)]
        source: String,

        /// Destination node
        #[arg(long)]
        destination: String,

        /// Grant consent (if not present, denies consent)
        #[arg(long, conflicts_with = "deny")]
        grant: bool,

        /// Deny consent
        #[arg(long, conflicts_with = "grant")]
        deny: bool,
    },

    /// Get provenance lineage for a resource
    GetReferences {
        /// Resource to query
        resource: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Note: host and port are currently not being used since trace2e_client
    // has hardcoded GRPC_URL. In production, you'd need to modify trace2e_client
    // to accept dynamic host/port configuration.
    let _host = &cli.host;
    let _port = cli.port;

    match cli.command {
        Commands::GetPolicies { resources } => {
            let parsed_resources: Result<Vec<_>> =
                resources.iter().map(|r| parse_resource(r)).collect();
            let resources = parsed_resources?;

            match o2m::get_policies(resources) {
                Ok(policies) => {
                    println!("Policies:");
                    for policy in policies {
                        println!("  {:?}", policy);
                    }
                    Ok(())
                }
                Err(e) => Err(anyhow!("Failed to get policies: {}", e)),
            }
        }

        Commands::SetPolicy { resource, confidentiality, integrity } => {
            let res = parse_resource(&resource)?;
            let conf = confidentiality.and_then(|c| {
                if c == 0 {
                    Some(primitives::Confidentiality::Public as i32)
                } else if c == 1 {
                    Some(primitives::Confidentiality::Secret as i32)
                } else {
                    None
                }
            });

            let policy = primitives::Policy {
                confidentiality: conf.unwrap_or(primitives::Confidentiality::Public as i32),
                integrity: integrity.unwrap_or(0),
                deleted: false,
                consent: false,
            };

            match o2m::set_policy(res, policy) {
                Ok(_) => {
                    println!("✓ Policy set for {}", resource);
                    Ok(())
                }
                Err(e) => Err(anyhow!("Failed to set policy: {}", e)),
            }
        }

        Commands::SetConfidentiality { resource, level } => {
            let res = parse_resource(&resource)?;
            let conf = if level == 0 {
                primitives::Confidentiality::Public
            } else {
                primitives::Confidentiality::Secret
            };

            match o2m::set_confidentiality(res, conf as i32) {
                Ok(_) => {
                    println!("✓ Confidentiality level {} set for {}", level, resource);
                    Ok(())
                }
                Err(e) => Err(anyhow!("Failed to set confidentiality: {}", e)),
            }
        }

        Commands::SetIntegrity { resource, level } => {
            let res = parse_resource(&resource)?;

            match o2m::set_integrity(res, level) {
                Ok(_) => {
                    println!("✓ Integrity level {} set for {}", level, resource);
                    Ok(())
                }
                Err(e) => Err(anyhow!("Failed to set integrity: {}", e)),
            }
        }

        Commands::SetDeleted { resource } => {
            let res = parse_resource(&resource)?;

            match o2m::set_deleted(res) {
                Ok(_) => {
                    println!("✓ Resource {} marked as deleted", resource);
                    Ok(())
                }
                Err(e) => Err(anyhow!("Failed to mark as deleted: {}", e)),
            }
        }

        Commands::BroadcastDeletion { resource } => {
            let res = parse_resource(&resource)?;

            match o2m::set_deleted(res) {
                Ok(_) => {
                    println!("✓ Deletion broadcast for {}", resource);
                    Ok(())
                }
                Err(e) => Err(anyhow!("Failed to broadcast deletion: {}", e)),
            }
        }

        Commands::EnforceConsent { resource } => {
            let res = parse_resource(&resource)?;

            println!("Listening for consent requests on {}...", resource);
            println!("Press Ctrl+C to stop.\n");

            match o2m::enforce_consent(res) {
                Ok(mut stream) => {
                    // Use tokio_stream to handle the async stream with cancellation support
                    while let Some(notification) = stream.next().await {
                        match notification {
                            Ok(ConsentNotification { consent_request }) => {
                                println!(
                                    "[{:?}] Consent Request: {}",
                                    tokio::time::Instant::now(),
                                    consent_request
                                );
                                println!(
                                    "      To approve: trace2e-operator set-consent-decision \\\n\
                                       --source <source> --destination <dest> --grant"
                                );
                                println!();
                            }
                            Err(e) => {
                                eprintln!("Stream error: {}", e);
                                return Err(anyhow!("Consent stream error: {}", e));
                            }
                        }
                    }
                    Ok(())
                }
                Err(e) => Err(anyhow!("Failed to enforce consent: {}", e)),
            }
        }

        Commands::SetConsentDecision { source, destination, grant, deny } => {
            let src = parse_resource(&source)?;
            let decision = if deny { false } else { grant };
            let dst = Destination::try_from(destination.as_str())
                .map_err(|e| anyhow!("Failed to parse destination: {}", e))?;
            match o2m::set_consent_decision(src, dst, decision) {
                Ok(_) => {
                    let status = if decision { "GRANTED" } else { "DENIED" };
                    println!("✓ Consent {} for {} → {}", status, source, destination);
                    Ok(())
                }
                Err(e) => Err(anyhow!("Failed to set consent decision: {}", e)),
            }
        }

        Commands::GetReferences { resource } => {
            let res = parse_resource(&resource)?;

            match o2m::get_references(res) {
                Ok(references) => {
                    println!("Provenance references for {}:", resource);
                    for ref_item in references {
                        println!("  {:?}", ref_item);
                    }
                    Ok(())
                }
                Err(e) => Err(anyhow!("Failed to get references: {}", e)),
            }
        }
    }
}
