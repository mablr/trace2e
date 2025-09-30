//! Consent service for managing user/operator consent for outgoing data flows.
use std::{future::Future, pin::Pin, sync::Arc, task::Poll};

use dashmap::{DashMap, Entry};
use tokio::{sync::broadcast, time::Duration};
use tower::Service;
#[cfg(feature = "trace2e_tracing")]
use tracing::info;

use crate::traceability::{
    api::{ConsentRequest, ConsentResponse},
    error::TraceabilityError,
    naming::Resource,
};

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
struct ConsentKey(Resource, Option<String>, Resource);

#[derive(Debug, Clone)]
enum ConsentState {
    Pending { tx: broadcast::Sender<bool> },
    Decided(bool),
}

#[derive(Default, Debug, Clone)]
pub struct ConsentService {
    timeout: u64,
    /// Unified store of consent states keyed by (source, node_id, destination)
    states: Arc<DashMap<ConsentKey, ConsentState>>,
}

impl ConsentService {
    /// Create a new `ConsentService` with the specified timeout.
    ///
    /// Timeout is disabled if set to 0.
    pub fn new(timeout_ms: u64) -> Self {
        Self { timeout: timeout_ms, states: Arc::new(DashMap::new()) }
    }

    /// Internal method to get consent
    async fn get_consent(
        &self,
        source: Resource,
        destination: (Option<String>, Resource),
    ) -> Result<bool, TraceabilityError> {
        let key = ConsentKey(source, destination.0, destination.1);
        match self.states.entry(key) {
            Entry::Occupied(occ) => match occ.get() {
                ConsentState::Decided(consent) => Ok(*consent),
                ConsentState::Pending { tx } => {
                    let mut rx = tx.subscribe();
                    // Drop map guard before awaiting
                    drop(occ);
                    if self.timeout == 0 {
                        rx.recv().await.map_err(|_| TraceabilityError::InternalTrace2eError)
                    } else {
                        match tokio::time::timeout(Duration::from_millis(self.timeout), rx.recv())
                            .await
                        {
                            Ok(res) => res.map_err(|_| TraceabilityError::InternalTrace2eError),
                            Err(_) => Err(TraceabilityError::ConsentRequestTimeout),
                        }
                    }
                }
            },
            Entry::Vacant(vac) => {
                let (tx, mut rx) = broadcast::channel(16);
                vac.insert(ConsentState::Pending { tx });
                if self.timeout == 0 {
                    rx.recv().await.map_err(|_| TraceabilityError::InternalTrace2eError)
                } else {
                    match tokio::time::timeout(Duration::from_millis(self.timeout), rx.recv()).await
                    {
                        Ok(res) => res.map_err(|_| TraceabilityError::InternalTrace2eError),
                        Err(_) => Err(TraceabilityError::ConsentRequestTimeout),
                    }
                }
            }
        }
    }

    /// Internal method to list pending requests
    fn pending_requests(
        &self,
    ) -> Vec<((Resource, Option<String>, Resource), broadcast::Sender<bool>)> {
        self.states
            .iter()
            .filter_map(|entry| match entry.value() {
                ConsentState::Pending { tx } => {
                    let ConsentKey(src, node, dst) = entry.key().clone();
                    Some(((src, node, dst), tx.clone()))
                }
                ConsentState::Decided(_) => None,
            })
            .collect()
    }

    /// Internal method to set consent
    fn set_consent(
        &self,
        source: Resource,
        destination: (Option<String>, Resource),
        consent: bool,
    ) {
        let key = ConsentKey(source, destination.0, destination.1);
        // Read-first: only upgrade to mutable if the state is Pending.
        if let Some(entry) = self.states.get(&key) {
            if matches!(entry.value(), ConsentState::Pending { .. }) {
                drop(entry);
                if let Some(mut entry) = self.states.get_mut(&key) {
                    if let ConsentState::Pending { tx } = entry.value() {
                        // Notify waiters, then transition to Decided.
                        tx.send(consent).unwrap();
                        *entry = ConsentState::Decided(consent);
                    }
                }
            }
        }
    }
}

impl Service<ConsentRequest> for ConsentService {
    type Response = ConsentResponse;
    type Error = TraceabilityError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: ConsentRequest) -> Self::Future {
        let this = self.clone();
        Box::pin(async move {
            match request {
                ConsentRequest::RequestConsent { source, destination } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[consent] RequestConsent: source: {:?}, destination: {:?}",
                        source, destination
                    );
                    let consent = this.get_consent(source, destination).await?;
                    Ok(ConsentResponse::Consent(consent))
                }
                ConsentRequest::PendingRequests => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!("[consent] PendingRequests");
                    Ok(ConsentResponse::PendingRequests(this.pending_requests()))
                }
                ConsentRequest::SetConsent { source, destination, consent } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[consent] SetConsent: source: {:?}, destination: {:?}, consent: {:?}",
                        source, destination, consent
                    );
                    this.set_consent(source, destination, consent);
                    Ok(ConsentResponse::Ack)
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tower::{Service, ServiceBuilder, timeout::TimeoutLayer};

    #[tokio::test]
    async fn unit_consent_service_pending_then_set() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut service = ConsentService::default();
        let source = Resource::new_process_mock(0);
        let dest = (Some("10.0.0.1".to_string()), Resource::new_file("/tmp/x".to_string()));

        // Start a RequestConsent that will await a decision
        let mut svc_for_req = service.clone();
        let src_clone = source.clone();
        let dest_clone = dest.clone();
        let waiter = tokio::spawn(async move {
            svc_for_req
                .call(ConsentRequest::RequestConsent { source: src_clone, destination: dest_clone })
                .await
        });

        // Give time for the pending entry to be created
        tokio::time::sleep(Duration::from_millis(5)).await;

        // Check that we have exactly one pending request
        let pending =
            service.call(ConsentRequest::PendingRequests).await.expect("pending requests call ok");
        if let ConsentResponse::PendingRequests(list) = pending {
            assert_eq!(list.len(), 1);
            let ((s, n, d), _tx) = &list[0];
            assert_eq!(s, &source);
            assert_eq!(n, &dest.0);
            assert_eq!(d, &dest.1);
        } else {
            panic!("Expected PendingRequests response");
        }

        // Decide consent and ensure waiter completes
        let res = service
            .call(ConsentRequest::SetConsent {
                source: source.clone(),
                destination: dest.clone(),
                consent: true,
            })
            .await
            .expect("set consent ok");
        assert!(matches!(res, ConsentResponse::Ack));

        let waited = waiter.await.unwrap().unwrap();
        assert!(matches!(waited, ConsentResponse::Consent(true)));
    }

    #[tokio::test]
    async fn unit_consent_service_decided_returns_immediately() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut service = ConsentService::default();
        let source = Resource::new_process_mock(1);
        let dest = (None, Resource::new_file("/tmp/y".to_string()));

        // First request will create pending, so set consent before awaiting a second request
        let mut svc_for_req = service.clone();
        let src_clone = source.clone();
        let dest_clone = dest.clone();
        let waiter = tokio::spawn(async move {
            svc_for_req
                .call(ConsentRequest::RequestConsent { source: src_clone, destination: dest_clone })
                .await
        });
        tokio::time::sleep(Duration::from_millis(5)).await;
        service
            .call(ConsentRequest::SetConsent {
                source: source.clone(),
                destination: dest.clone(),
                consent: false,
            })
            .await
            .expect("set consent ok");
        // First waiter should get false
        assert!(matches!(waiter.await.unwrap().unwrap(), ConsentResponse::Consent(false)));

        // Second request should return immediately with the decided value
        let immediate = service
            .call(ConsentRequest::RequestConsent { source, destination: dest })
            .await
            .unwrap();
        assert!(matches!(immediate, ConsentResponse::Consent(false)));
    }

    #[tokio::test]
    async fn unit_consent_service_times_out_with_layer() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();

        // Wrap with a larger timeout tower layer
        let mut svc = ServiceBuilder::new()
            .layer(TimeoutLayer::new(Duration::from_millis(2)))
            .service(ConsentService::new(1));

        let source = Resource::new_process_mock(2);
        let destination = (None, Resource::new_file("/tmp/timeout.txt".to_string()));

        // This call should pend internally and then error with ConsentRequestTimeout
        let result = svc.call(ConsentRequest::RequestConsent { source, destination }).await;

        match result {
            Ok(_) => panic!("expected timeout error, got Ok"),
            Err(err) => {
                // TimeoutLayer boxes inner errors; downcast to our TraceabilityError
                if let Some(te) = err.downcast_ref::<TraceabilityError>() {
                    assert_eq!(*te, TraceabilityError::ConsentRequestTimeout);
                } else if err.is::<tower::timeout::error::Elapsed>() {
                    panic!("outer TimeoutLayer elapsed before inner consent timeout");
                } else {
                    panic!("unexpected error: {err}");
                }
            }
        }
    }
}
