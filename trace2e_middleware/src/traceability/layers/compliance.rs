use std::{future::Future, pin::Pin, task::Poll};

use tower::Service;

use crate::traceability::{
    error::TraceabilityError,
    message::{TraceabilityRequest, TraceabilityResponse},
};

#[derive(Clone, Default)]
pub struct ComplianceService;

impl Service<TraceabilityRequest> for ComplianceService {
    type Response = TraceabilityResponse;
    type Error = TraceabilityError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: TraceabilityRequest) -> Self::Future {
        Box::pin(async move {
            match req.clone() {
                // todo : implement compliance check service
                TraceabilityRequest::Request { .. } => Ok(TraceabilityResponse::Grant),
                TraceabilityRequest::Report { .. } => Ok(TraceabilityResponse::Ack),
            }
        })
    }
}
