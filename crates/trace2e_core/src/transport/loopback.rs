use std::{
    collections::VecDeque,
    future::Future,
    pin::Pin,
    sync::Arc,
    task::Poll,
    time::{Duration, Instant},
};

use dashmap::DashMap;
use tower::Service;

use crate::{
    traceability::{
        M2mApiDefaultStack, O2mApiDefaultStack, P2mApiDefaultStack,
        api::{M2mRequest, M2mResponse},
        error::TraceabilityError,
        init_middleware,
    },
    transport::eval_remote_ip,
};

pub async fn spawn_loopback_middlewares(
    ips: Vec<String>,
) -> VecDeque<(P2mApiDefaultStack<M2mLoopback>, O2mApiDefaultStack)> {
    spawn_loopback_middlewares_with_entropy(ips, 0, 0).await
}

pub async fn spawn_loopback_middlewares_with_entropy(
    ips: Vec<String>,
    base_delay_ms: u64,
    jitter_max_ms: u64,
) -> VecDeque<(P2mApiDefaultStack<M2mLoopback>, O2mApiDefaultStack)> {
    let m2m_loopback = M2mLoopback::new(base_delay_ms, jitter_max_ms);
    let mut middlewares = VecDeque::new();
    for ip in ips {
        let (m2m, p2m, o2m) =
            init_middleware(ip.clone(), None, Default::default(), m2m_loopback.clone());
        m2m_loopback.register_middleware(ip.clone(), m2m).await;
        middlewares.push_back((p2m, o2m));
    }
    middlewares
}

#[derive(Clone)]
pub struct M2mLoopback {
    middlewares: Arc<DashMap<String, M2mApiDefaultStack>>,
    base_delay_ms: u64,
    jitter_max_ms: u64,
    last_call_time: Arc<std::sync::Mutex<Option<Instant>>>,
}

impl Default for M2mLoopback {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

impl M2mLoopback {
    pub fn new(base_delay_ms: u64, jitter_max_ms: u64) -> Self {
        Self {
            middlewares: Arc::new(DashMap::new()),
            base_delay_ms,
            jitter_max_ms,
            last_call_time: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    pub async fn register_middleware(&self, ip: String, middleware: M2mApiDefaultStack) {
        self.middlewares.insert(ip, middleware);
    }

    pub async fn get_middleware(
        &self,
        ip: String,
    ) -> Result<M2mApiDefaultStack, TraceabilityError> {
        self.middlewares
            .get(&ip)
            .map(|c| c.to_owned())
            .ok_or(TraceabilityError::TransportFailedToContactRemote(ip))
    }

    fn calculate_delay(&self) -> Duration {
        if self.base_delay_ms == 0 && self.jitter_max_ms == 0 {
            return Duration::from_millis(0);
        }

        let base = Duration::from_millis(self.base_delay_ms);
        if self.jitter_max_ms == 0 {
            return base;
        }

        // Simple entropy based on current time
        let seed = Instant::now().elapsed().as_nanos() as u64;
        let jitter_ms = seed % (self.jitter_max_ms + 1);
        base + Duration::from_millis(jitter_ms)
    }
}

impl Service<M2mRequest> for M2mLoopback {
    type Response = M2mResponse;
    type Error = TraceabilityError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        let now = Instant::now();
        let delay = self.calculate_delay();

        if delay.is_zero() {
            return Poll::Ready(Ok(()));
        }

        // Check if we need to wait based on last call time
        if let Ok(mut last_time) = self.last_call_time.lock() {
            match *last_time {
                Some(last) if now.duration_since(last) < delay => {
                    // Still need to wait
                    let waker = cx.waker().clone();
                    let remaining = delay - now.duration_since(last);
                    tokio::spawn(async move {
                        tokio::time::sleep(remaining).await;
                        waker.wake();
                    });
                    return Poll::Pending;
                }
                _ => {
                    // Update last call time
                    *last_time = Some(now);
                }
            }
        }

        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: M2mRequest) -> Self::Future {
        let this = self.clone();
        Box::pin(async move {
            this.get_middleware(eval_remote_ip(request.clone())?).await?.call(request).await
        })
    }
}
