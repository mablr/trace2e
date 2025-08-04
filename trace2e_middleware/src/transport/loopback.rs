use std::{collections::HashMap, pin::Pin, sync::Arc, task::Poll};
use tokio::sync::Mutex;
use tower::Service;

use crate::{
    traceability::{
        M2mApiDefaultStack, P2mApiDefaultStack,
        api::{M2mRequest, M2mResponse},
        error::TraceabilityError,
        init_middleware,
    },
    transport::eval_remote_ip,
};

pub async fn spawn_loopback_middlewares(ips: Vec<String>) -> Vec<P2mApiDefaultStack<M2mLoopback>> {
    let m2m_loopback = M2mLoopback::default();
    let mut middlewares = Vec::new();
    for ip in ips {
        let (m2m, p2m) = init_middleware(None, m2m_loopback.clone());
        m2m_loopback.register_middleware(ip.clone(), m2m).await;
        middlewares.push(p2m);
    }
    middlewares
}

#[derive(Clone, Default)]
pub struct M2mLoopback {
    middlewares: Arc<Mutex<HashMap<String, M2mApiDefaultStack>>>,
}

impl M2mLoopback {
    pub async fn register_middleware(&self, ip: String, middleware: M2mApiDefaultStack) {
        self.middlewares.lock().await.insert(ip, middleware);
    }

    pub async fn get_middleware(
        &self,
        ip: String,
    ) -> Result<M2mApiDefaultStack, TraceabilityError> {
        self.middlewares
            .lock()
            .await
            .get(&ip)
            .cloned()
            .ok_or(TraceabilityError::TransportFailedToContactRemote(ip))
    }
}

impl Service<M2mRequest> for M2mLoopback {
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
