use std::{pin::Pin, task::Poll};

use tower::Service;

use super::{
    error::TraceabilityError,
    message::{ResourceRequest, ResourceResponse},
};

#[derive(Debug, Default, Clone)]
pub struct ResourceService;

impl Service<ResourceRequest> for ResourceService {
    type Response = ResourceResponse;
    type Error = TraceabilityError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, req: ResourceRequest) -> Self::Future {
        // Dummy implementation for now
        // later this will return compliance labels
        Box::pin(async move {
            Ok(match req {
                ResourceRequest::ReadRequest => ResourceResponse::Ack,
                ResourceRequest::WriteRequest => ResourceResponse::Ack,
                ResourceRequest::ReadReport => ResourceResponse::Ack,
                ResourceRequest::WriteReport => ResourceResponse::Ack,
            })
        })
    }
}
