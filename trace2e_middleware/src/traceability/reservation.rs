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
    message::{ReservationRequest, ReservationResponse},
};

#[derive(Default, Debug, Clone)]
pub enum ReservationState {
    #[default]
    Available,
    Shared(usize),
    Exclusive,
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

    fn poll_ready(&mut self, ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.reservation_service.poll_ready(ctx)
    }

    fn call(&mut self, request: ReservationRequest) -> Self::Future {
        let reservation_service_clone = self.reservation_service.clone();
        let mut inner = std::mem::replace(&mut self.reservation_service, reservation_service_clone);
        let self_clone = self.clone();
        Box::pin(async move {
            let result = inner.call(request).await;
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
struct ReservationService {
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

    use tokio::time::sleep;
    use tower::{ServiceBuilder, layer::layer_fn, timeout::TimeoutLayer};

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
}
