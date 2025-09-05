use std::{collections::HashSet, pin::Pin, task::Poll};

use tower::Service;

use crate::traceability::{
    api::{M2mRequest, M2mResponse},
    core::compliance::Policy,
    error::TraceabilityError,
};

#[derive(Clone, Default)]
pub struct M2mNop;

impl Service<M2mRequest> for M2mNop {
    type Response = M2mResponse;
    type Error = TraceabilityError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: M2mRequest) -> Self::Future {
        Box::pin(async move {
            Ok(match request {
                M2mRequest::GetDestinationCompliance { .. } => {
                    M2mResponse::DestinationCompliance(Policy::default())
                }
                M2mRequest::GetSourceCompliance { .. } => {
                    M2mResponse::SourceCompliance(HashSet::new())
                }
                M2mRequest::UpdateProvenance { .. } => M2mResponse::Ack,
            })
        })
    }
}
