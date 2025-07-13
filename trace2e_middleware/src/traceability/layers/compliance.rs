use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc, task::Poll};

use tokio::sync::Mutex;
use tower::Service;

use crate::traceability::{
    error::TraceabilityError,
    message::{TraceabilityRequest, TraceabilityResponse},
    naming::Identifier,
};

#[derive(Default, PartialEq)]
enum ConfidentialityPolicy {
    Secret,
    #[default]
    Public,
}

#[derive(Default)]
struct Policy {
    confidentiality: ConfidentialityPolicy,
    integrity: u8,
}

#[derive(Default, Clone)]
pub struct ComplianceService {
    policies: Arc<Mutex<HashMap<Identifier, Policy>>>,
}

impl ComplianceService {
    async fn is_compliant(&self, source: Identifier, destination: Identifier) -> bool {
        let policies = self.policies.lock().await;

        let default_policy = Policy::default();
        let source_policy = policies.get(&source).unwrap_or(&default_policy);
        let destination_policy = policies.get(&destination).unwrap_or(&default_policy);

        // Integrity check: Source integrity must be greater than or equal to destination integrity
        if source_policy.integrity < destination_policy.integrity {
            return false;
        }

        // Confidentiality check: Secret data cannot flow to public destinations
        if source_policy.confidentiality == ConfidentialityPolicy::Secret
            && destination_policy.confidentiality == ConfidentialityPolicy::Public
        {
            return false;
        }

        true
    }
}

impl Service<TraceabilityRequest> for ComplianceService {
    type Response = TraceabilityResponse;
    type Error = TraceabilityError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: TraceabilityRequest) -> Self::Future {
        let this = self.clone();
        Box::pin(async move {
            match req.clone() {
                // todo : implement compliance check service
                TraceabilityRequest::Request {
                    source,
                    destination,
                } => {
                    if this.is_compliant(source, destination).await {
                        Ok(TraceabilityResponse::Grant)
                    } else {
                        Err(TraceabilityError::DirectPolicyViolation)
                    }
                }
                TraceabilityRequest::Report { .. } => Ok(TraceabilityResponse::Ack),
            }
        })
    }
}
