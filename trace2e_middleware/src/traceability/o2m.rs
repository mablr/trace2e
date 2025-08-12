use std::{future::Future, pin::Pin, task::Poll};
use tower::Service;
#[cfg(feature = "trace2e_tracing")]
use tracing::info;

use crate::traceability::{
    api::{
        ComplianceRequest, ComplianceResponse, O2mRequest, O2mResponse, ProvenanceRequest,
        ProvenanceResponse,
    },
    error::TraceabilityError,
    naming::NodeId,
};

#[derive(Debug, Clone)]
pub struct O2mApiService<P, C> {
    provenance: P,
    compliance: C,
}

impl<P, C> O2mApiService<P, C> {
    pub fn new(provenance: P, compliance: C) -> Self {
        Self {
            provenance,
            compliance,
        }
    }
}

impl<P, C> Service<O2mRequest> for O2mApiService<P, C>
where
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
    type Response = O2mResponse;
    type Error = TraceabilityError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: O2mRequest) -> Self::Future {
        let mut provenance = self.provenance.clone();
        let mut compliance = self.compliance.clone();
        Box::pin(async move {
            match request {
                O2mRequest::GetPolicies(resources) => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[o2m-{}] GetPolicies: resources: {:?}",
                        provenance.node_id(),
                        resources
                    );
                    match compliance
                        .call(ComplianceRequest::GetPolicies(resources))
                        .await?
                    {
                        ComplianceResponse::Policies(policies) => {
                            Ok(O2mResponse::Policies(policies))
                        }
                        _ => Err(TraceabilityError::InternalTrace2eError),
                    }
                }
                O2mRequest::SetPolicy { resource, policy } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[o2m-{}] SetPolicy: resource: {:?}, policy: {:?}",
                        provenance.node_id(),
                        resource,
                        policy
                    );
                    match compliance
                        .call(ComplianceRequest::SetPolicy { resource, policy })
                        .await?
                    {
                        ComplianceResponse::PolicyUpdated => Ok(O2mResponse::Ack),
                        _ => Err(TraceabilityError::InternalTrace2eError),
                    }
                }
                O2mRequest::GetReferences(resource) => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[o2m-{}] GetReferences: resource: {:?}",
                        provenance.node_id(),
                        resource
                    );
                    match provenance
                        .call(ProvenanceRequest::GetReferences(resource))
                        .await?
                    {
                        ProvenanceResponse::Provenance { references, .. } => {
                            Ok(O2mResponse::References(references))
                        }
                        _ => Err(TraceabilityError::InternalTrace2eError),
                    }
                }
            }
        })
    }
}
