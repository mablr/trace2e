//! Request validation and filtering for traceability operations.
//!
//! This module provides validation mechanisms to ensure that incoming requests
//! contain valid, accessible resources before they are processed by the core
//! traceability services. It acts as a gatekeeper to prevent invalid operations
//! from reaching the business logic layer.
//!
//! ## Validation Categories
//!
//! **Process Validation**: Verifies that process IDs refer to actual running processes
//! by querying the system process table.
//!
//! **Stream Validation**: Ensures that socket addresses are well-formed and compatible
//! (e.g., both IPv4 or both IPv6) for network stream operations.
//!
//! ## Integration
//!
//! The validator is designed to work with the P2M service through the embedded validation
//! feature. Invalid requests are rejected early with descriptive error messages before
//! consuming system resources.

use std::net::SocketAddr;

use sysinfo::{Pid, System};

/// Resource validator for P2M requests.
///
/// This validator ensures that incoming Process-to-Middleware requests reference
/// valid, accessible system resources before they are processed by the core services.
/// It performs system-level validation that cannot be done at the type level.
///
/// ## Validation Rules
///
/// - **Process IDs**: Must correspond to currently running processes
/// - **Socket Addresses**: Must be parseable and use compatible address families
/// - **File Descriptors**: Accepted without validation (OS handles validity)
///
/// ## Usage
///
/// The validator methods are called by the P2M service when resource validation
/// is enabled through the `with_resource_validation(true)` builder method.
#[derive(Debug, Clone)]
pub struct ResourceValidator;

impl ResourceValidator {
    /// Validates that a process ID corresponds to a currently running process.
    ///
    /// Queries the system process table to verify that the specified PID
    /// is associated with an active process. This prevents operations on
    /// stale or invalid process identifiers.
    ///
    /// # Arguments
    /// * `pid` - Process identifier to validate
    ///
    /// # Returns
    /// `true` if the process exists and is accessible, `false` otherwise
    pub fn is_valid_process(&self, pid: i32) -> bool {
        let mut system = System::new();
        system.refresh_all();
        system.process(Pid::from(pid as usize)).is_some()
    }

    /// Validates that socket addresses are well-formed and compatible.
    ///
    /// Ensures that both local and peer socket addresses can be parsed
    /// and use compatible address families (both IPv4 or both IPv6).
    /// This prevents network operations with mismatched or invalid addresses.
    ///
    /// # Arguments
    /// * `local_socket` - Local socket address string
    /// * `peer_socket` - Peer socket address string
    ///
    /// # Returns
    /// `true` if both addresses are valid and compatible, `false` otherwise
    pub fn is_valid_stream(&self, local_socket: &str, peer_socket: &str) -> bool {
        match (local_socket.parse::<SocketAddr>(), peer_socket.parse::<SocketAddr>()) {
            (Ok(local_socket), Ok(peer_socket)) => {
                (local_socket.is_ipv4() && peer_socket.is_ipv4())
                    || (local_socket.is_ipv6() && peer_socket.is_ipv6())
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use tower::Service;

    use crate::{
        traceability::{
            api::{P2mRequest, P2mResponse},
            core::{
                compliance::ComplianceService, provenance::ProvenanceService,
                sequencer::SequencerService,
            },
            p2m::P2mApiService,
        },
        transport::nop::M2mNop,
    };

    #[tokio::test]
    async fn unit_traceability_provenance_service_p2m_validator() {
        let mut p2m_service = P2mApiService::new(
            SequencerService::default(),
            ProvenanceService::default(),
            ComplianceService::default(),
            M2mNop,
        )
        .with_resource_validation(true);

        assert_eq!(
            p2m_service
                .call(P2mRequest::LocalEnroll { pid: 1, fd: 1, path: "test".to_string() })
                .await
                .unwrap(),
            P2mResponse::Ack
        );

        assert_eq!(
            p2m_service
                .call(P2mRequest::RemoteEnroll {
                    pid: 1,
                    local_socket: "127.0.0.1:8080".to_string(),
                    peer_socket: "127.0.0.1:8081".to_string(),
                    fd: 1
                })
                .await
                .unwrap(),
            P2mResponse::Ack
        );

        let P2mResponse::Grant(flow_id) =
            p2m_service.call(P2mRequest::IoRequest { pid: 1, fd: 1, output: true }).await.unwrap()
        else {
            panic!("Expected P2mResponse::Grant");
        };
        assert_eq!(
            p2m_service
                .call(P2mRequest::IoReport { pid: 1, fd: 1, grant_id: flow_id, result: true })
                .await
                .unwrap(),
            P2mResponse::Ack
        );

        assert_eq!(
            p2m_service
                .call(P2mRequest::LocalEnroll { pid: 0, fd: 1, path: "test".to_string() }) // pid 0 is invalid
                .await
                .unwrap_err()
                .to_string(),
            "Traceability error, process not found (pid: 0)"
        );

        assert_eq!(
            p2m_service
                .call(P2mRequest::RemoteEnroll {
                    pid: 1,
                    local_socket: "bad_socket".to_string(),
                    peer_socket: "bad_socket".to_string(),
                    fd: 1
                })
                .await
                .unwrap_err()
                .to_string(),
            "Traceability error, invalid stream (local_socket: bad_socket, peer_socket: bad_socket)"
        );
    }
}
