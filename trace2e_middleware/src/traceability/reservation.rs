use std::{
    collections::VecDeque,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};
use tokio::sync::oneshot;
use tower::Service;

use super::{
    error::ReservationError,
    message::{ReservationRequest, ReservationResponse, ResourceRequest, ResourceResponse},
};

pub struct GuardedResource<S, R> {
    inner: S,
    reservation: R,
}

impl<S, R> GuardedResource<S, R> {
    pub fn new(inner: S, reservation: R) -> Self {
        Self { inner, reservation }
    }
}

impl<S, R> Service<ResourceRequest> for GuardedResource<S, R>
where
    S: Service<ResourceRequest, Response = ResourceResponse> + Clone + Send + 'static,
    S::Error: From<ReservationError> + From<R::Error> + Send + 'static,
    S::Future: Send + 'static,
    R: Service<ReservationRequest, Response = ReservationResponse> + Clone + Send + 'static,
    R::Error: From<ReservationError> + Send + 'static,
    R::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: ResourceRequest) -> Self::Future {
        let mut inner = self.inner.clone(); // Assuming S: Clone
        let mut reservation = self.reservation.clone();

        Box::pin(async move {
            match req {
                ResourceRequest::ReadRequest => {
                    reservation.call(ReservationRequest::GetShared).await?;
                    inner.call(ResourceRequest::ReadRequest).await
                }

                ResourceRequest::WriteRequest => {
                    reservation.call(ReservationRequest::GetExclusive).await?;
                    inner.call(ResourceRequest::WriteRequest).await
                }

                ResourceRequest::ReadReport => {
                    let res = inner.call(ResourceRequest::ReadReport).await?;
                    reservation.call(ReservationRequest::ReleaseShared).await?;
                    Ok(res)
                }

                ResourceRequest::WriteReport => {
                    let res = inner.call(ResourceRequest::WriteReport).await?;
                    reservation
                        .call(ReservationRequest::ReleaseExclusive)
                        .await?;
                    Ok(res)
                }
            }
        })
    }
}

#[derive(Default, Debug, Clone)]
struct WaitingQueueService<S> {
    reservation_service: S,
    waiting_queue: Arc<Mutex<VecDeque<oneshot::Sender<()>>>>,
}

impl<S> WaitingQueueService<S> {
    fn new(inner: S) -> Self {
        Self {
            reservation_service: inner,
            waiting_queue: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    fn add_to_waiting_queue(&self) -> Result<oneshot::Receiver<()>, ReservationError> {
        let (tx, rx) = oneshot::channel();
        self.waiting_queue
            .lock()
            .map_err(|_| ReservationError::ReservationWaitingQueueError)?
            .push_back(tx); // TODO: make waiting queue specific error ?
        Ok(rx)
    }

    fn notify_waiting_requests(&self) {
        if let Ok(mut waiting_queue) = self.waiting_queue.lock() {
            if let Some(request) = waiting_queue.pop_front() {
                let _ = request.send(());
            }
        } // Mutex and Channel errors are ignored
    }
}

impl<S> Service<ReservationRequest> for WaitingQueueService<S>
where
    S: Service<ReservationRequest, Response = ReservationResponse, Error = ReservationError>
        + Clone
        + Send
        + 'static,
    S::Future: Send,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.reservation_service.poll_ready(cx)
    }

    fn call(&mut self, request: ReservationRequest) -> Self::Future {
        let reservation_service_clone = self.reservation_service.clone();
        let mut inner = std::mem::replace(&mut self.reservation_service, reservation_service_clone);
        let self_clone = self.clone();
        Box::pin(async move {
            let result = inner.call(request.clone()).await;
            match result {
                Ok(response) => {
                    if matches!(
                        request,
                        ReservationRequest::ReleaseShared | ReservationRequest::ReleaseExclusive
                    ) && response == ReservationResponse::Released
                    {
                        self_clone.notify_waiting_requests();
                    }
                    Ok(response)
                }
                Err(ReservationError::AlreadyReservedExclusive)
                | Err(ReservationError::AlreadyReservedShared) => {
                    let rx = self_clone.add_to_waiting_queue()?;
                    let _ = rx.await;
                    inner.call(request).await
                }
                Err(e) => Err(e),
            }
        })
    }
}

#[derive(Default, Debug, Clone)]
pub enum ReservationState {
    #[default]
    Available,
    Shared(usize),
    Exclusive,
}

#[derive(Default, Debug, Clone)]
pub struct ReservationService {
    state: Arc<Mutex<ReservationState>>,
}

impl Service<ReservationRequest> for ReservationService {
    type Response = ReservationResponse;
    type Error = ReservationError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: ReservationRequest) -> Self::Future {
        let state = self.state.clone();
        Box::pin(async move {
            if let Ok(mut state) = state.lock() {
                match request {
                    ReservationRequest::GetShared => match *state {
                        ReservationState::Available => {
                            *state = ReservationState::Shared(1);
                            Ok(ReservationResponse::Reserved)
                        }
                        ReservationState::Shared(n) => {
                            *state = ReservationState::Shared(n + 1);
                            Ok(ReservationResponse::Reserved)
                        }
                        ReservationState::Exclusive => {
                            Err(ReservationError::AlreadyReservedExclusive)
                        }
                    },
                    ReservationRequest::GetExclusive => match *state {
                        ReservationState::Available => {
                            *state = ReservationState::Exclusive;
                            Ok(ReservationResponse::Reserved)
                        }
                        ReservationState::Shared(_) => Err(ReservationError::AlreadyReservedShared),
                        ReservationState::Exclusive => {
                            Err(ReservationError::AlreadyReservedExclusive)
                        }
                    },
                    ReservationRequest::ReleaseShared => match *state {
                        ReservationState::Available => Ok(ReservationResponse::Released),
                        ReservationState::Shared(n) => {
                            if n > 1 {
                                *state = ReservationState::Shared(n - 1);
                                Ok(ReservationResponse::Reserved)
                            } else {
                                *state = ReservationState::Available;
                                Ok(ReservationResponse::Released)
                            }
                        }
                        ReservationState::Exclusive => Err(ReservationError::UnauthorizedRelease),
                    },
                    ReservationRequest::ReleaseExclusive => match *state {
                        ReservationState::Available => Ok(ReservationResponse::Released),
                        ReservationState::Shared(_) => Err(ReservationError::UnauthorizedRelease),
                        ReservationState::Exclusive => {
                            *state = ReservationState::Available;
                            Ok(ReservationResponse::Released)
                        }
                    },
                }
            } else {
                Err(ReservationError::ReservationLockError)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time::{sleep, timeout};
    use tower::{ServiceBuilder, layer::layer_fn, service_fn, timeout::TimeoutLayer};

    use super::*;

    #[tokio::test]
    async fn unit_traceability_reservation_service_release() {
        let mut reservation_service = ServiceBuilder::new().service(ReservationService::default());

        // Release on available
        assert!(
            reservation_service
                .call(ReservationRequest::ReleaseShared)
                .await
                .is_ok()
        );

        assert!(
            reservation_service
                .call(ReservationRequest::ReleaseExclusive)
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn unit_traceability_reservation_service_release_exclusive_unauthorized() {
        let mut reservation_service = ServiceBuilder::new().service(ReservationService::default());

        // Release on available
        assert!(
            reservation_service
                .call(ReservationRequest::GetShared)
                .await
                .is_ok()
        );

        assert!(
            reservation_service
                .call(ReservationRequest::ReleaseExclusive)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn unit_traceability_reservation_service_release_shared_unauthorized() {
        let mut reservation_service = ServiceBuilder::new().service(ReservationService::default());

        // Release on available
        assert!(
            reservation_service
                .call(ReservationRequest::GetExclusive)
                .await
                .is_ok()
        );

        assert!(
            reservation_service
                .call(ReservationRequest::ReleaseShared)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn unit_traceability_reservation_service_exclusive() {
        let mut reservation_service = ServiceBuilder::new().service(ReservationService::default());

        // Exclusive reservation
        assert!(
            reservation_service
                .call(ReservationRequest::GetExclusive)
                .await
                .is_ok()
        );
        assert!(
            reservation_service
                .call(ReservationRequest::GetExclusive)
                .await
                .is_err()
        );
        assert!(
            reservation_service
                .call(ReservationRequest::GetShared)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn unit_traceability_reservation_service_exclusive_release() {
        let mut reservation_service = ServiceBuilder::new().service(ReservationService::default());

        // Exclusive reservation
        assert!(
            reservation_service
                .call(ReservationRequest::GetExclusive)
                .await
                .is_ok()
        );
        assert!(
            reservation_service
                .call(ReservationRequest::ReleaseExclusive)
                .await
                .is_ok()
        );
        assert!(
            reservation_service
                .call(ReservationRequest::GetExclusive)
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn unit_traceability_reservation_service_shared() {
        let mut reservation_service = ServiceBuilder::new().service(ReservationService::default());

        // Shared reservation
        assert!(
            reservation_service
                .call(ReservationRequest::GetShared)
                .await
                .is_ok()
        );
        assert!(
            reservation_service
                .call(ReservationRequest::GetShared)
                .await
                .is_ok()
        );
        assert!(
            reservation_service
                .call(ReservationRequest::GetExclusive)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn unit_traceability_reservation_service_shared_release() {
        let mut reservation_service = ServiceBuilder::new().service(ReservationService::default());

        // Release after shared
        assert!(
            reservation_service
                .call(ReservationRequest::GetShared)
                .await
                .is_ok()
        );
        assert!(
            reservation_service
                .call(ReservationRequest::GetShared)
                .await
                .is_ok()
        );
        assert!(
            reservation_service
                .call(ReservationRequest::ReleaseShared)
                .await
                .is_ok()
        );
        assert!(
            reservation_service
                .call(ReservationRequest::GetExclusive)
                .await
                .is_err()
        );
        assert!(
            reservation_service
                .call(ReservationRequest::ReleaseShared)
                .await
                .is_ok()
        );
        assert!(
            reservation_service
                .call(ReservationRequest::GetExclusive)
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn unit_traceability_reservation_service_waiting_queue_timeout_shared_exclusive() {
        let timeout_layer = TimeoutLayer::new(Duration::from_millis(1));
        let waiting_queue_layer = layer_fn(|service| WaitingQueueService::new(service));
        let mut reservation_service = ServiceBuilder::new()
            .layer(timeout_layer)
            .layer(waiting_queue_layer)
            .service(ReservationService::default());

        assert!(
            reservation_service
                .call(ReservationRequest::GetShared)
                .await
                .is_ok()
        );

        assert_eq!(
            reservation_service
                .call(ReservationRequest::GetExclusive)
                .await
                .unwrap_err()
                .to_string(),
            "request timed out"
        );
    }

    #[tokio::test]
    async fn unit_traceability_reservation_service_waiting_queue_timeout_exclusive_shared() {
        let timeout_layer = TimeoutLayer::new(Duration::from_millis(1));
        let waiting_queue_layer = layer_fn(|service| WaitingQueueService::new(service));
        let mut reservation_service = ServiceBuilder::new()
            .layer(timeout_layer)
            .layer(waiting_queue_layer)
            .service(ReservationService::default());

        assert!(
            reservation_service
                .call(ReservationRequest::GetExclusive)
                .await
                .is_ok()
        );

        assert_eq!(
            reservation_service
                .call(ReservationRequest::GetShared)
                .await
                .unwrap_err()
                .to_string(),
            "request timed out"
        );
    }

    #[tokio::test]
    async fn unit_traceability_reservation_service_waiting_queue_delayed_release() {
        let timeout_layer = TimeoutLayer::new(Duration::from_micros(10));
        let waiting_queue_layer = layer_fn(|service| WaitingQueueService::new(service));
        let mut reservation_service = ServiceBuilder::new()
            .layer(timeout_layer)
            .layer(waiting_queue_layer)
            .service(ReservationService::default());
        assert!(
            reservation_service
                .call(ReservationRequest::GetShared)
                .await
                .is_ok()
        );
        let mut reservation_service_clone = reservation_service.clone();
        tokio::spawn(async move {
            sleep(Duration::from_micros(11)).await;
            reservation_service_clone
                .call(ReservationRequest::ReleaseShared)
                .await
                .unwrap();
        });
        assert_eq!(
            reservation_service
                .call(ReservationRequest::GetExclusive)
                .await
                .unwrap_err()
                .to_string(),
            "request timed out"
        );
        sleep(Duration::from_micros(2)).await;
        assert!(
            reservation_service
                .call(ReservationRequest::GetExclusive)
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn unit_traceability_reservation_middleware() {
        let reservation_service = ServiceBuilder::new()
            .layer(layer_fn(|service| WaitingQueueService::new(service)))
            .service(ReservationService::default());
        let resource_mock_service = service_fn(|req: ResourceRequest| async move {
            Ok::<ResourceResponse, ReservationError>(match req {
                ResourceRequest::ReadRequest => ResourceResponse::Grant,
                ResourceRequest::WriteRequest => ResourceResponse::Grant,
                ResourceRequest::ReadReport => ResourceResponse::Ack,
                ResourceRequest::WriteReport => ResourceResponse::Ack,
            })
        });
        let mut resource_service = GuardedResource::new(resource_mock_service, reservation_service);
        assert!(
            resource_service
                .call(ResourceRequest::ReadRequest)
                .await
                .is_ok()
        );
        assert!(
            resource_service
                .call(ResourceRequest::ReadRequest)
                .await
                .is_ok()
        );

        // It will wait for the exclusive reservation (already taken by the first read requests)
        assert!(
            timeout(
                Duration::from_millis(1),
                resource_service.call(ResourceRequest::WriteRequest)
            )
            .await
            .is_err()
        );

        assert!(
            resource_service
                .call(ResourceRequest::ReadReport)
                .await
                .is_ok()
        );

        // It will wait for the exclusive reservation (not all read requests are done)
        assert!(
            timeout(
                Duration::from_millis(1),
                resource_service.call(ResourceRequest::WriteRequest)
            )
            .await
            .is_err()
        );

        assert!(
            resource_service
                .call(ResourceRequest::ReadReport)
                .await
                .is_ok()
        );

        // Now the exclusive reservation can be taken, write request will be granted
        assert!(
            resource_service
                .call(ResourceRequest::WriteRequest)
                .await
                .is_ok()
        );
    }
}
