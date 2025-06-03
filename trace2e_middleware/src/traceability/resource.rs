use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use tower::{Service, ServiceBuilder, layer::layer_fn};

use super::{
    error::TraceabilityError,
    message::{ReservationRequest, ResourceRequest, ResourceResponse},
    reservation::{ReservationService, WaitingQueueService},
};

#[derive(Debug, Clone)]
pub struct GuardedResourceService {
    inner: ResourceService,
    reservation: WaitingQueueService<ReservationService>,
}

impl GuardedResourceService {
    pub fn new() -> Self {
        Self {
            reservation: ServiceBuilder::new()
                .layer(layer_fn(|service| WaitingQueueService::new(service)))
                .service(ReservationService::default()),
            inner: ServiceBuilder::new().service(ResourceService::default()),
        }
    }
}

impl Service<ResourceRequest> for GuardedResourceService {
    type Response = ResourceResponse;
    type Error = TraceabilityError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: ResourceRequest) -> Self::Future {
        let mut inner = self.inner.clone(); // Assuming S: Clone
        let mut reservation = self.reservation.clone();

        Box::pin(async move {
            match req {
                ResourceRequest::ReleaseShared => {
                    reservation.call(ReservationRequest::ReleaseShared).await?;
                    inner.call(ResourceRequest::ReleaseShared).await
                }
                ResourceRequest::ReleaseExclusive => {
                    reservation
                        .call(ReservationRequest::ReleaseExclusive)
                        .await?;
                    inner.call(ResourceRequest::ReleaseExclusive).await
                }
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
                ResourceRequest::ReleaseShared => ResourceResponse::Ack,
                ResourceRequest::ReleaseExclusive => ResourceResponse::Ack,
                ResourceRequest::ReadRequest => ResourceResponse::Ack,
                ResourceRequest::WriteRequest => ResourceResponse::Ack,
                ResourceRequest::ReadReport => ResourceResponse::Ack,
                ResourceRequest::WriteReport => ResourceResponse::Ack,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time::timeout;
    use tower::ServiceBuilder;

    use super::*;

    #[tokio::test]
    async fn unit_traceability_reservation_middleware() {
        let mut resource_service = ServiceBuilder::new().service(GuardedResourceService::new());
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
