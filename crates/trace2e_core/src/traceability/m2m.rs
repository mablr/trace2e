use std::{future::Future, pin::Pin, task::Poll};

use tower::Service;
#[cfg(feature = "trace2e_tracing")]
use tracing::info;

use crate::traceability::{
    api::{
        ComplianceRequest, ComplianceResponse, M2mRequest, M2mResponse, ProvenanceRequest,
        ProvenanceResponse, SequencerRequest, SequencerResponse,
    },
    error::TraceabilityError,
    naming::NodeId,
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
                M2mRequest::GetDestinationCompliance { source, destination } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[m2m-{}] GetDestinationCompliance: source: {:?}, destination: {:?}",
                        provenance.node_id(),
                        source,
                        destination
                    );
                    match sequencer
                        .call(SequencerRequest::ReserveFlow {
                            source,
                            destination: destination.clone(),
                        })
                        .await?
                    {
                        SequencerResponse::FlowReserved => {
                            match compliance.call(ComplianceRequest::GetPolicy(destination)).await?
                            {
                                ComplianceResponse::Policy(policy) => {
                                    Ok(M2mResponse::DestinationCompliance(policy))
                                }
                                _ => Err(TraceabilityError::InternalTrace2eError),
                            }
                        }
                        _ => Err(TraceabilityError::InternalTrace2eError),
                    }
                }
                M2mRequest::GetSourceCompliance { resources, .. } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[m2m-{}] GetSourceCompliance: resources: {:?}",
                        provenance.node_id(),
                        resources
                    );
                    match compliance.call(ComplianceRequest::GetPolicies(resources)).await? {
                        ComplianceResponse::Policies(policies) => {
                            Ok(M2mResponse::SourceCompliance(policies))
                        }
                        _ => Err(TraceabilityError::InternalTrace2eError),
                    }
                }
                M2mRequest::UpdateProvenance { source_prov, destination } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[m2m-{}] UpdateProvenance: source_prov: {:?}, destination: {:?}",
                        provenance.node_id(),
                        source_prov,
                        destination
                    );
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
            }
        })
    }
}
