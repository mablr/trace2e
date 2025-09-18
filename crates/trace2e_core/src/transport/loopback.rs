//! # Loopback Transport Implementation
//!
//! This module provides a loopback transport implementation for testing and
//! development scenarios where multiple middleware instances need to communicate
//! within a single process or test environment. It simulates network communication
//! by routing M2M requests directly to local middleware instances.
//!
//! ## Features
//!
//! - **In-Process Communication**: Routes calls directly to registered middleware instances
//! - **Network Simulation**: Supports configurable delays and jitter to simulate network latency
//! - **Test Orchestration**: Provides utilities for spawning multiple middleware instances
//! - **Resource Pre-enrollment**: Supports pre-populating middleware with test resources
//!
//! ## Use Cases
//!
//! - Unit and integration testing of distributed traceability scenarios
//! - Development and debugging of multi-node workflows
//! - Performance testing with controlled network conditions
//! - Simulation of distributed systems in a single process
//!
//! ## Network Simulation
//!
//! The loopback transport can simulate network characteristics by introducing
//! configurable delays and jitter to M2M calls, allowing testing of timeout
//! handling and performance under various network conditions.

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
        init_middleware_with_enrolled_resources,
    },
    transport::eval_remote_ip,
};

/// Spawns multiple loopback middleware instances with no pre-enrolled resources.
///
/// Creates a set of middleware instances that can communicate with each other
/// through the loopback transport. Each middleware is identified by an IP address
/// for routing purposes.
///
/// # Arguments
///
/// * `ips` - Vector of IP addresses to assign to the middleware instances
///
/// # Returns
///
/// A queue of (P2M service, O2M service) tuples for each middleware instance.
pub async fn spawn_loopback_middlewares(
    ips: Vec<String>,
) -> VecDeque<(P2mApiDefaultStack<M2mLoopback>, O2mApiDefaultStack)> {
    spawn_loopback_middlewares_with_enrolled_resources(ips, 0, 0, 0).await
}

/// Spawns multiple loopback middleware instances with pre-enrolled test resources.
///
/// Creates middleware instances and pre-populates them with the specified number
/// of processes, files, and network streams for testing purposes.
///
/// # Arguments
///
/// * `ips` - Vector of IP addresses for the middleware instances
/// * `process_count` - Number of processes to enroll per middleware
/// * `per_process_file_count` - Number of files to create per process
/// * `per_process_stream_count` - Number of network streams to create per process
pub async fn spawn_loopback_middlewares_with_enrolled_resources(
    ips: Vec<String>,
    process_count: u32,
    per_process_file_count: u32,
    per_process_stream_count: u32,
) -> VecDeque<(P2mApiDefaultStack<M2mLoopback>, O2mApiDefaultStack)> {
    spawn_loopback_middlewares_with_entropy(
        ips,
        0,
        0,
        process_count,
        per_process_file_count,
        per_process_stream_count,
    )
    .await
}

/// Spawns loopback middleware instances with network simulation and pre-enrolled resources.
///
/// Creates middleware instances with configurable network delay simulation and
/// pre-populated test resources. This is the most comprehensive setup function
/// for testing complex distributed scenarios.
///
/// # Arguments
///
/// * `ips` - Vector of IP addresses for the middleware instances
/// * `base_delay_ms` - Base network delay in milliseconds
/// * `jitter_max_ms` - Maximum additional random delay in milliseconds
/// * `process_count` - Number of processes to enroll per middleware
/// * `per_process_file_count` - Number of files to create per process
/// * `per_process_stream_count` - Number of network streams to create per process
pub async fn spawn_loopback_middlewares_with_entropy(
    ips: Vec<String>,
    base_delay_ms: u64,
    jitter_max_ms: u64,
    process_count: u32,
    per_process_file_count: u32,
    per_process_stream_count: u32,
) -> VecDeque<(P2mApiDefaultStack<M2mLoopback>, O2mApiDefaultStack)> {
    let m2m_loopback = M2mLoopback::new(base_delay_ms, jitter_max_ms);
    let mut middlewares = VecDeque::new();
    for ip in ips {
        let (m2m, p2m, o2m) = init_middleware_with_enrolled_resources(
            ip.clone(),
            None,
            m2m_loopback.clone(),
            process_count,
            per_process_file_count,
            per_process_stream_count,
        );
        m2m_loopback.register_middleware(ip.clone(), m2m).await;
        middlewares.push_back((p2m, o2m));
    }
    middlewares
}

/// Loopback transport service for in-process M2M communication.
///
/// `M2mLoopback` provides a transport implementation that routes M2M requests
/// to local middleware instances within the same process. It maintains a
/// registry of middleware instances indexed by IP address and supports
/// configurable network delay simulation.
///
/// ## Network Simulation
///
/// The service can simulate network latency by introducing delays before
/// processing requests. This includes both a base delay and random jitter
/// to simulate real network conditions.
///
/// ## Thread Safety
///
/// The service is thread-safe and can be safely cloned and used across
/// multiple concurrent tasks. All internal state is protected by appropriate
/// synchronization primitives.
#[derive(Clone)]
pub struct M2mLoopback {
    /// Registry of middleware instances indexed by IP address.
    middlewares: Arc<DashMap<String, M2mApiDefaultStack>>,
    /// Base network delay in milliseconds.
    base_delay_ms: u64,
    /// Maximum additional random delay in milliseconds.
    jitter_max_ms: u64,
    /// Timestamp of the last call for delay calculation.
    last_call_time: Arc<std::sync::Mutex<Option<Instant>>>,
}

impl Default for M2mLoopback {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

impl M2mLoopback {
    /// Creates a new loopback transport with the specified delay characteristics.
    ///
    /// # Arguments
    ///
    /// * `base_delay_ms` - Base delay to add to all requests in milliseconds
    /// * `jitter_max_ms` - Maximum random additional delay in milliseconds
    pub fn new(base_delay_ms: u64, jitter_max_ms: u64) -> Self {
        Self {
            middlewares: Arc::new(DashMap::new()),
            base_delay_ms,
            jitter_max_ms,
            last_call_time: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Registers a middleware instance with the specified IP address.
    ///
    /// This allows the loopback transport to route requests to the appropriate
    /// middleware instance based on the target IP address extracted from requests.
    ///
    /// # Arguments
    ///
    /// * `ip` - IP address identifier for the middleware
    /// * `middleware` - The middleware service instance to register
    pub async fn register_middleware(&self, ip: String, middleware: M2mApiDefaultStack) {
        self.middlewares.insert(ip, middleware);
    }

    /// Retrieves a middleware instance for the specified IP address.
    ///
    /// # Arguments
    ///
    /// * `ip` - IP address of the target middleware
    ///
    /// # Returns
    ///
    /// The middleware service instance, or an error if not found.
    ///
    /// # Errors
    ///
    /// Returns `TransportFailedToContactRemote` if no middleware is registered
    /// for the specified IP address.
    pub async fn get_middleware(
        &self,
        ip: String,
    ) -> Result<M2mApiDefaultStack, TraceabilityError> {
        self.middlewares
            .get(&ip)
            .map(|c| c.to_owned())
            .ok_or(TraceabilityError::TransportFailedToContactRemote(ip))
    }

    /// Calculates the delay to apply based on the configured delay parameters.
    ///
    /// Combines the base delay with a random jitter component to simulate
    /// realistic network latency characteristics.
    ///
    /// # Returns
    ///
    /// The total delay duration to apply to the current request.
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
