use crate::traceability::{
    api::{
        ComplianceRequest, ComplianceResponse, M2mRequest, M2mResponse, ProvenanceRequest,
        ProvenanceResponse, SequencerRequest, SequencerResponse,
    },
    error::TraceabilityError,
};
use std::{future::Future, pin::Pin, task::Poll};
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
        let this = self.clone();
        let mut sequencer = std::mem::replace(&mut self.sequencer, this.sequencer.clone());
        let mut provenance = std::mem::replace(&mut self.provenance, this.provenance.clone());
        let mut compliance = std::mem::replace(&mut self.compliance, this.compliance.clone());
        Box::pin(async move {
            match request {
                M2mRequest::ComplianceRetrieval {
                    source,
                    destination,
                } => {
                    match sequencer
                        .call(SequencerRequest::ReserveFlow {
                            source: source.clone(),
                            destination: destination.clone(),
                        })
                        .await
                    {
                        Ok(SequencerResponse::FlowReserved) => {
                            match compliance
                                .call(ComplianceRequest::GetPolicies {
                                    ids: [destination.clone()].into(),
                                })
                                .await
                            {
                                Ok(ComplianceResponse::Policies(policies)) => {
                                    Ok(M2mResponse::Compliance {
                                        destination: policies
                                            .get(&destination)
                                            .cloned()
                                            .unwrap_or_default(),
                                    })
                                }
                                Err(e) => Err(e),
                                _ => Err(TraceabilityError::InternalTrace2eError),
                            }
                        }
                        Err(e) => Err(e),
                        _ => Err(TraceabilityError::InternalTrace2eError),
                    }
                }
                M2mRequest::ProvenanceUpdate {
                    source,
                    source_prov,
                    destination,
                } => {
                    match provenance
                        .call(ProvenanceRequest::UpdateProvenanceRaw {
                            source_prov,
                            destination: destination.clone(),
                        })
                        .await
                    {
                        Ok(ProvenanceResponse::ProvenanceUpdated)
                        | Ok(ProvenanceResponse::ProvenanceNotUpdated) => {
                            match sequencer
                                .call(SequencerRequest::ReleaseFlow {
                                    source,
                                    destination,
                                })
                                .await
                            {
                                Ok(SequencerResponse::FlowReleased) => Ok(M2mResponse::Ack),
                                Err(e) => Err(e),
                                _ => Err(TraceabilityError::InternalTrace2eError),
                            }
                        }
                        Err(e) => Err(e),
                        _ => Err(TraceabilityError::InternalTrace2eError),
                    }
                }
            }
        })
    }
}
