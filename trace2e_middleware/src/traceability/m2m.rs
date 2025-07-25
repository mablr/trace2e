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
    node_id: String,
    sequencer: S,
    provenance: P,
    compliance: C,
}

impl<S, P, C> M2mApiService<S, P, C> {
    pub fn new(sequencer: S, provenance: P, compliance: C) -> Self {
        Self::new_with_node_id(String::new(), sequencer, provenance, compliance)
    }

    pub fn new_with_node_id(node_id: String, sequencer: S, provenance: P, compliance: C) -> Self {
        Self {
            node_id,
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
        let mut _sequencer = std::mem::replace(&mut self.sequencer, this.sequencer.clone());
        let mut _provenance = std::mem::replace(&mut self.provenance, this.provenance.clone());
        let mut _compliance = std::mem::replace(&mut self.compliance, this.compliance.clone());
        Box::pin(async move {
            match request {
                M2mRequest::IoRequest { .. } => todo!(),
                M2mRequest::IoReport { .. } => todo!(),
            }
        })
    }
}
