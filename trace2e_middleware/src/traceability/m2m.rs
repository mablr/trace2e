use crate::traceability::{
    api::{
        ComplianceRequest, ComplianceResponse, M2mRequest, M2mResponse, ProvenanceRequest,
        ProvenanceResponse, SequencerRequest, SequencerResponse,
    },
    error::TraceabilityError,
};
use std::{collections::HashSet, future::Future, pin::Pin, task::Poll};
use tower::Service;

#[derive(Debug, Clone)]
pub struct M2mApiService<S, P, C> {
    sequencer: S,
    provenance: P,
    compliance: C,
}

impl<S, P, C> M2mApiService<S, P, C> {
    pub fn new(sequencer: S, provenance: P, compliance: C) -> Self {
        Self {
            sequencer,
            provenance,
            compliance,
        }
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
                M2mRequest::GetConsistentCompliance {
                    source,
                    destination,
                } => {
                    match sequencer
                        .call(SequencerRequest::ReserveFlow {
                            source,
                            destination: destination.clone(),
                        })
                        .await?
                    {
                        SequencerResponse::FlowReserved => {
                            match compliance
                                .call(ComplianceRequest::GetPolicies(HashSet::from([
                                    destination.clone()
                                ])))
                                .await?
                            {
                                ComplianceResponse::Policies(policies) => {
                                    Ok(M2mResponse::Compliance(policies))
                                }
                                _ => Err(TraceabilityError::InternalTrace2eError),
                            }
                        }
                        _ => Err(TraceabilityError::InternalTrace2eError),
                    }
                }
                M2mRequest::GetLooseCompliance { resources, .. } => {
                    match compliance
                        .call(ComplianceRequest::GetPolicies(resources))
                        .await?
                    {
                        ComplianceResponse::Policies(policies) => {
                            Ok(M2mResponse::Compliance(policies))
                        }
                        _ => Err(TraceabilityError::InternalTrace2eError),
                    }
                }
                M2mRequest::UpdateProvenance {
                    source_prov,
                    destination,
                } => {
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
