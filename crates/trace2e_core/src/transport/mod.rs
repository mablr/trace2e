//! # Transport Module
//!
//! This module provides the networking and communication layer for the trace2e framework,
//! enabling distributed traceability operations between middleware instances across
//! different nodes in a networked environment.
//!
//! ## Overview
//!
//! The transport layer abstracts the underlying communication mechanisms used for
//! machine-to-machine (M2M) interactions in the trace2e system. It supports multiple
//! transport implementations including gRPC for production environments, loopback and
//! no-op for testing purposes.
//!
//! ## Transport Implementations
//!
//! - **gRPC Transport**: Production-ready implementation using Protocol Buffers and gRPC
//! - **Loopback Transport**: Local testing implementation that routes calls internally
//!   between mock middleware instances
//! - **No-op Transport**: Stub implementation that performs no actual communication
//!
//! ## Remote IP Evaluation
//!
//! The module provides utilities for extracting remote IP addresses from M2M requests,
//! enabling proper routing and connection management for distributed operations.
//!
//! ## Architecture
//!
//! The transport layer operates between the traceability middleware instances,
//! facilitating the exchange of compliance policies, provenance information,
//! and authorization decisions across network boundaries.

use std::net::SocketAddr;

use crate::traceability::{
    api::types::M2mRequest,
    error::TraceabilityError,
    infrastructure::naming::{Fd, Resource},
};

pub mod grpc;
pub mod loopback;
pub mod nop;

/// Extracts the remote IP address from an M2M request for routing purposes.
///
/// This function analyzes the request type and destination resource to determine
/// the appropriate remote IP address that should handle the request. For stream
/// resources, it extracts the IP from the local socket address. For source
/// compliance requests, it uses the provided authority IP.
///
/// # Arguments
///
/// * `req` - The M2M request to analyze
///
/// # Returns
///
/// The remote IP address as a string, or an error if the IP cannot be determined.
///
/// # Errors
///
/// Returns `TransportFailedToEvaluateRemote` if:
/// - The destination is not a stream resource
/// - The socket address cannot be parsed
/// - The request type doesn't contain routing information
pub fn eval_remote_ip(req: M2mRequest) -> Result<String, TraceabilityError> {
    match req {
        M2mRequest::GetDestinationCompliance { destination, .. }
        | M2mRequest::UpdateProvenance { destination, .. } => match destination {
            // Local socket is the remote socket of the remote stream
            Resource::Fd(Fd::Stream(stream)) => match stream.local_socket.parse::<SocketAddr>() {
                Ok(addr) => Ok(addr.ip().to_string()),
                Err(_) => Err(TraceabilityError::TransportFailedToEvaluateRemote),
            },
            _ => Err(TraceabilityError::TransportFailedToEvaluateRemote),
        },
        M2mRequest::GetSourceCompliance { authority_ip, .. } => Ok(authority_ip),
    }
}
