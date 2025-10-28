//! # No-Op Transport Implementation
//!
//! This module provides a stub transport implementation that handles M2M requests
//! without performing any actual network communication. It's designed for scenarios
//! where distributed functionality is disabled or not needed, such as single-node
//! deployments or testing isolated middleware behavior.
//!
//! ## Behavior
//!
//! The no-op transport responds to all M2M requests with sensible default values:
//!
//! - **Destination Compliance**: Returns default policy (public, integrity 0, not deleted, consent given)
//! - **Source Compliance**: Returns empty policy map (no source policies)
//! - **Update Provenance**: Acknowledges the request without action
//!
//! ## Use Cases
//!
//! - Single-node deployments where distributed features are unnecessary
//! - Testing middleware logic in isolation without network dependencies
//! - Fallback transport when network communication is disabled
//! - Development environments where distributed setup is not feasible

use std::{collections::HashMap, future::Future, pin::Pin, task::Poll};

use tower::Service;

use crate::traceability::{
    api::types::{M2mRequest, M2mResponse},
    error::TraceabilityError,
    services::compliance::Policy,
};

/// No-operation transport service for M2M communication.
///
/// `M2mNop` provides a stub implementation of the M2M transport interface
/// that responds to all requests with default values without performing
/// any actual network communication or distributed coordination.
///
/// ## Response Behavior
///
/// - **GetDestinationCompliance**: Always returns a default policy
/// - **GetSourceCompliance**: Always returns an empty policy map
/// - **UpdateProvenance**: Always acknowledges without action
///
/// This transport is useful for single-node deployments or testing
/// scenarios where distributed functionality is not required.
#[derive(Clone, Default)]
pub struct M2mNop;

/// Implementation of the M2M service interface for the no-op transport.
///
/// This implementation handles all M2M request types by returning appropriate
/// default responses without performing any actual distributed operations.
/// All requests complete immediately and successfully.
impl Service<M2mRequest> for M2mNop {
    type Response = M2mResponse;
    type Error = TraceabilityError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    /// Always reports ready to handle requests immediately.
    fn poll_ready(&mut self, _: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    /// Handles M2M requests by returning appropriate default responses.
    ///
    /// # Request Handling
    ///
    /// - **GetDestinationPolicy**: Returns a default policy with public
    ///   confidentiality, zero integrity, not deleted, and consent given
    /// - **CheckSourceCompliance**: Returns an empty policy map indicating
    ///   no source policies are available
    /// - **UpdateProvenance**: Acknowledges the request without performing
    ///   any provenance updates
    fn call(&mut self, request: M2mRequest) -> Self::Future {
        Box::pin(async move {
            Ok(match request {
                M2mRequest::GetDestinationPolicy { .. } => {
                    M2mResponse::DestinationPolicy(Policy::default())
                }
                M2mRequest::CheckSourceCompliance { .. }
                | M2mRequest::UpdateProvenance { .. }
                | M2mRequest::BroadcastDeletion(_) => M2mResponse::Ack,
            })
        })
    }
}
