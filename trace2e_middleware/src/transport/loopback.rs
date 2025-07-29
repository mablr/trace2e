use std::{pin::Pin, task::Poll};
use tower::Service;

use crate::traceability::{
    api::{M2mRequest, M2mResponse},
    error::TraceabilityError,
};

#[derive(Clone, Default)]
pub struct M2mLoopback<M> {
    m2m: M,
}

impl<M> Service<M2mRequest> for M2mLoopback<M>
where
    M: Service<M2mRequest, Response = M2mResponse, Error = TraceabilityError>
        + Clone
        + Send
        + 'static,
    M::Future: Send,
{
    type Response = M2mResponse;
    type Error = TraceabilityError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: M2mRequest) -> Self::Future {
        let mut this = self.clone();
        Box::pin(async move { this.m2m.call(request).await })
    }
}
