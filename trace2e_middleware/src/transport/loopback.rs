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

    pub async fn get_middleware(&self, ip: String) -> Result<M, TraceabilityError> {
        self.middlewares
            .lock()
            .await
            .get(&ip)
            .cloned()
            .ok_or(TraceabilityError::TransportFailedToContactRemote(ip))
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
            this.get_middleware(eval_remote_ip(request.clone())?)
                .await?
                .call(request)
                .await
        })
    }
}
