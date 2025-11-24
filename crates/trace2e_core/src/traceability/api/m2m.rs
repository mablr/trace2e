//! Middleware-to-Middleware (M2M) API service implementation.
//!
//! This module provides the service implementation for communication between distributed
//! middleware instances in the traceability network. The M2M API enables consistent
//! compliance checking, flow coordination, and provenance synchronization across
//! multiple nodes in geographically distributed deployments.
//!
//! ## Service Architecture
//!
//! The `M2mApiService` coordinates between three core services:
//! - **Sequencer Service**: For flow reservation and coordination across nodes
//! - **Provenance Service**: For provenance tracking and ancestry synchronization
//! - **Compliance Service**: For distributed policy evaluation and enforcement
//!
//! ## Distributed Operations
//!
//! **Cross-Node Compliance**: Query and evaluate policies across middleware boundaries
//! to ensure consistent enforcement of organizational and regulatory requirements.
//!
//! **Flow Coordination**: Reserve and release distributed flows to prevent race
//! conditions and ensure atomic operations across the network.
//!
//! **Provenance Synchronization**: Transfer provenance data between nodes to maintain
//! complete audit trails for cross-boundary data flows.
//!
//! ## Network Considerations
//!
//! M2M operations involve network communication and may experience latency or failures.
//! The service handles these conditions gracefully and provides appropriate error
//! responses for downstream handling.

use std::{collections::HashSet, future::Future, pin::Pin, task::Poll};

use tower::Service;

use crate::traceability::infrastructure::naming::DisplayableResource;
use tracing::info;

use crate::traceability::{
    api::types::{
        ComplianceRequest, ComplianceResponse, M2mRequest, M2mResponse, ProvenanceRequest,
        ProvenanceResponse, SequencerRequest, SequencerResponse,
    },
    error::TraceabilityError,
    infrastructure::naming::NodeId,
};

/// M2M (Middleware-to-Middleware) API Service
///
/// This service handles communication between distributed middleware instances,
/// enabling consistent compliance checking, flow coordination, and provenance updates
/// across multiple nodes in the traceability network.
#[derive(Debug, Clone)]
pub struct M2mApiService<S, P, C> {
    /// Service for managing flows sequencing
    sequencer: S,
    /// Service for tracking resources provenance
    provenance: P,
    /// Service for policy management and compliance checking
    compliance: C,
}

impl<S, P, C> M2mApiService<S, P, C> {
    /// Creates a new M2M API service with the provided sequencer, provenance, and compliance
    /// services
    pub fn new(sequencer: S, provenance: P, compliance: C) -> Self {
        Self { sequencer, provenance, compliance }
    }
}

impl<S, P, C> Service<M2mRequest> for M2mApiService<S, P, C>
where
    S: Service<SequencerRequest, Response = SequencerResponse, Error = TraceabilityError>
        + Clone
        + Send
        + 'static,
    S::Future: Send,
    P: Service<ProvenanceRequest, Response = ProvenanceResponse, Error = TraceabilityError>
        + Clone
        + Send
        + NodeId
        + 'static,
    P::Future: Send,
    C: Service<ComplianceRequest, Response = ComplianceResponse, Error = TraceabilityError>
        + Clone
        + Send
        + 'static,
    C::Future: Send,
{
    type Response = M2mResponse;
    type Error = TraceabilityError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: M2mRequest) -> Self::Future {
        let mut sequencer = self.sequencer.clone();
        let mut provenance = self.provenance.clone();
        let mut compliance = self.compliance.clone();
        Box::pin(async move {
            match request {
                M2mRequest::GetDestinationPolicy(destination) => {
                    info!(
                        node_id = %provenance.node_id(),
                        destination = %destination,
                        "[m2m] GetDestinationPolicy"
                    );
                    // check if the destination is local
                    let destination = if *destination.node_id() == provenance.node_id() {
                        destination.resource().to_owned()
                    } else {
                        return Err(TraceabilityError::NotLocalResource);
                    };
                    match compliance.call(ComplianceRequest::GetPolicy(destination)).await? {
                        ComplianceResponse::Policy(policy) => {
                            Ok(M2mResponse::DestinationPolicy(policy))
                        }
                        _ => Err(TraceabilityError::InternalTrace2eError),
                    }
                }
                M2mRequest::CheckSourceCompliance { sources, destination } => {
                    info!(
                        node_id = %provenance.node_id(),
                        sources = %DisplayableResource::from(&sources),
                        destination = %destination.0,
                        destination_policy = ?destination.1,
                        "[m2m] CheckSourceCompliance"
                    );
                    let sources = sources
                        .iter()
                        .filter(|r| *r.node_id() == provenance.node_id())
                        .map(|r| r.resource().to_owned())
                        .collect::<HashSet<_>>();
                    match compliance
                        .call(ComplianceRequest::EvalCompliance {
                            sources,
                            destination: destination.0,
                            destination_policy: Some(destination.1),
                        })
                        .await
                    {
                        Ok(ComplianceResponse::Grant) => Ok(M2mResponse::Ack),
                        Err(e) => Err(e),
                        _ => Err(TraceabilityError::InternalTrace2eError),
                    }
                }
                M2mRequest::UpdateProvenance { source_prov, destination } => {
                    info!(
                        node_id = %provenance.node_id(),
                        source_prov = %DisplayableResource::from(&source_prov),
                        destination = %destination,
                        "[m2m] UpdateProvenance"
                    );
                    // check if the destination is local
                    let destination = if *destination.node_id() == provenance.node_id() {
                        destination.resource().to_owned()
                    } else {
                        return Err(TraceabilityError::NotLocalResource);
                    };
                    match provenance
                        .call(ProvenanceRequest::UpdateProvenanceRaw {
                            source_prov,
                            destination: destination.clone(),
                        })
                        .await?
                    {
                        ProvenanceResponse::ProvenanceUpdated
                        | ProvenanceResponse::ProvenanceNotUpdated => {
                            match sequencer
                                .call(SequencerRequest::ReleaseFlow { destination })
                                .await?
                            {
                                SequencerResponse::FlowReleased { .. } => Ok(M2mResponse::Ack),
                                _ => Err(TraceabilityError::InternalTrace2eError),
                            }
                        }
                        _ => Err(TraceabilityError::InternalTrace2eError),
                    }
                }
                M2mRequest::BroadcastDeletion(resource) => {
                    info!(
                        node_id = %provenance.node_id(),
                        resource = %resource,
                        "[m2m] BroadcastDeletion"
                    );
                    // check if the resource is local
                    if *resource.node_id() == provenance.node_id() {
                        match compliance
                            .call(ComplianceRequest::SetDeleted(resource.resource().to_owned()))
                            .await?
                        {
                            ComplianceResponse::PolicyUpdated => Ok(M2mResponse::Ack),
                            _ => Err(TraceabilityError::InternalTrace2eError),
                        }
                    } else {
                        // If the resource is not local, just return Ack, as no action is needed here.
                        Ok(M2mResponse::Ack)
                    }
                }
            }
        })
    }
}
