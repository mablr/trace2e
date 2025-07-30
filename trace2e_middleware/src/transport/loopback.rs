use std::{collections::HashMap, pin::Pin, sync::Arc, task::Poll};
use tokio::sync::Mutex;
use tower::Service;

use crate::{
    traceability::{
        api::{M2mRequest, M2mResponse},
        error::TraceabilityError,
    },
    transport::eval_remote_ip,
};

#[derive(Clone, Default)]
pub struct M2mLoopback<M> {
    middlewares: Arc<Mutex<HashMap<String, M>>>,
}

impl<M> M2mLoopback<M>
where
    M: Clone,
{
    pub async fn register_middleware(&self, ip: String, middleware: M) {
        self.middlewares.lock().await.insert(ip, middleware);
    }

    pub async fn get_middleware(&self, ip: String) -> Option<M> {
        self.middlewares.lock().await.get(&ip).cloned()
    }
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
        let this = self.clone();
        Box::pin(async move {
            let Some(remote_ip) = eval_remote_ip(request.clone()) else {
                return Err(TraceabilityError::TransportFailedToEvaluateRemote);
            };
            let Some(mut middleware) = this.get_middleware(remote_ip.clone()).await else {
                return Err(TraceabilityError::TransportFailedToContactRemote(remote_ip));
            };
            middleware.call(request).await
        })
    }
}
