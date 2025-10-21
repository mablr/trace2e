//! Consent service for managing consent for outgoing data flows of resources.
use std::{future::Future, pin::Pin, sync::Arc, task::Poll};

use dashmap::{DashMap, Entry};
use tokio::{sync::broadcast, time::Duration};
use tower::Service;
#[cfg(feature = "trace2e_tracing")]
use tracing::info;

use crate::traceability::{error::TraceabilityError, infrastructure::naming::Resource};

/// Consent service request types.
///
/// API for the consent service, which manages user consent for data flow operations.
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum ConsentRequest {
    /// Request consent for a data flow operation.
    ///
    /// Requests consent from the resource owner for a data flow operation.
    RequestConsent {
        /// Source resource providing data
        source: Resource,
        /// Destination resource receiving data
        destination: Destination,
    },
    /// Take ownership of a resource.
    ///
    /// The owner of the resource will be able to receive consent request notifications
    /// and send back decisions for the resource through the returned channels.
    TakeResourceOwnership(Resource),
    /// Set consent decision for a specific data flow operation.
    ///
    /// Updates the consent status for a pending data flow operation.
    SetConsent {
        /// Source resource providing data
        source: Resource,
        /// Destination resource receiving data
        destination: Destination,
        /// Consent decision: true to grant, false to deny
        consent: bool,
    },
}

/// Consent service response types.
///
/// Responses from the consent service regarding consent decisions and pending requests.
#[derive(Debug)]
pub enum ConsentResponse {
    /// Consent granted or denied for a data flow.
    Consent(bool),
    /// Acknowledgment of successful consent decision update.
    Ack,
    /// Notification channel for the resource.
    Notifications(broadcast::Receiver<Destination>),
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Destination {
    Resource(Option<String>, Resource),
    Node(String),
    None, // Convenience for no destination
}

impl Destination {
    pub fn new(node_id: Option<String>, resource: Option<Resource>) -> Self {
        match (node_id, resource) {
            (node_id, Some(resource)) => Self::Resource(node_id, resource),
            (Some(node_id), None) => Self::Node(node_id),
            (None, None) => Self::None,
        }
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
struct ConsentKey(Resource, Destination);

#[derive(Default, Debug, Clone)]

pub struct ConsentService {
    timeout: u64,
    /// Unified store of consent states keyed by (source, node_id, destination)
    states: Arc<DashMap<ConsentKey, bool>>,
    /// Consent request notification channels
    notifications_channels: Arc<DashMap<Resource, broadcast::Sender<Destination>>>,
    /// Consent decision channels
    decision_channels: Arc<DashMap<ConsentKey, broadcast::Sender<bool>>>,
}

impl ConsentService {
    /// Create a new `ConsentService` with the specified timeout.
    ///
    /// Timeout is disabled if set to 0.
    pub fn new(timeout_ms: u64) -> Self {
        Self {
            timeout: timeout_ms,
            states: Arc::new(DashMap::new()),
            notifications_channels: Arc::new(DashMap::new()),
            decision_channels: Arc::new(DashMap::new()),
        }
    }

    /// Internal method to get consent
    async fn get_consent(
        &self,
        source: Resource,
        destination: Destination,
    ) -> Result<bool, TraceabilityError> {
        let key = ConsentKey(source, destination);
        if let Some(consent) = self.states.get(&key) {
            Ok(*consent)
        } else if let Some(notif_feed) = self.notifications_channels.get(&key.0) {
            // Send consent request notification
            // Then subscribe to decision channel if it exists or create a new one
            let mut decision_rx = if let Some(decision_feed) = self.decision_channels.get(&key) {
                notif_feed.send(key.1).map_err(|_| TraceabilityError::InternalTrace2eError)?;
                decision_feed.subscribe()
            } else {
                let (tx, rx) = broadcast::channel(100);
                self.decision_channels.insert(key.clone(), tx);
                notif_feed.send(key.1).map_err(|_| TraceabilityError::InternalTrace2eError)?;
                rx
            };
            // Handle timeout
            if self.timeout > 0 {
                tokio::time::timeout(Duration::from_millis(self.timeout), decision_rx.recv())
                    .await
                    .map_err(|_| TraceabilityError::ConsentRequestTimeout)?
                    .map_err(|_| TraceabilityError::InternalTrace2eError)
            } else {
                decision_rx.recv().await.map_err(|_| TraceabilityError::InternalTrace2eError)
            }
        } else {
            // No notifications feed, so nobody will ever know about this consent request
            // and it will never be granted
            Ok(false)
        }
    }

    /// Internal method to set consent
    fn set_consent(&self, source: Resource, destination: Destination, consent: bool) {
        let key = ConsentKey(source, destination);
        if self.states.insert(key.clone(), consent).is_none()
            && let Some((_, decision_feed)) = self.decision_channels.remove(&key)
        {
            // Ignore send errors as the receiver might have been dropped due to timeout
            let _ = decision_feed.send(consent);
        }
    }

    /// Internal method to subscribe to consent request notifications
    fn take_resource_ownership(&self, resource: Resource) -> broadcast::Receiver<Destination> {
        match self.notifications_channels.entry(resource.clone()) {
            Entry::Occupied(entry) => entry.get().subscribe(),
            Entry::Vacant(entry) => {
                let (tx, rx) = broadcast::channel(100);
                entry.insert(tx);
                rx
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
                        "[consent] RequestConsent from source: {:?} to destination: {:?}",
                        source, destination
                    );
                    this.get_consent(source, destination).await.map(ConsentResponse::Consent)
                }
                ConsentRequest::SetConsent { source, destination, consent } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[consent] SetConsent {} from source: {:?} to destination: {:?}",
                        consent, source, destination
                    );
                    this.set_consent(source, destination, consent);
                    Ok(ConsentResponse::Ack)
                }
                ConsentRequest::TakeResourceOwnership(resource) => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!("[consent] TakeResourceOwnership for resource: {:?}", resource);
                    Ok(ConsentResponse::Notifications(this.take_resource_ownership(resource)))
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_consent_service_no_ownership() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut consent_service = ConsentService::new(0);
        let resource = Resource::new_process_mock(0);
        let destination = Resource::new_file("/tmp/test.txt".to_string());
        let request = ConsentRequest::RequestConsent {
            source: resource.clone(),
            destination: Destination::new(None, Some(destination.clone())),
        };
        let response = consent_service.call(request).await.unwrap();
        // No ownership, so no consent can be granted
        assert!(matches!(response, ConsentResponse::Consent(false)));
    }

    #[tokio::test]
    async fn test_consent_service_with_ownership_with_decision_on_notification() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut consent_service = ConsentService::new(0);
        let resource = Resource::new_process_mock(0);
        let destination =
            Destination::new(None, Some(Resource::new_file("/tmp/test.txt".to_string())));
        let request = ConsentRequest::TakeResourceOwnership(resource.clone());
        let ownership_response = consent_service.call(request).await.unwrap();
        let ConsentResponse::Notifications(mut notifications_feed) = ownership_response else {
            panic!("Expected Notifications");
        };
        let resource_clone = resource.clone();
        let destination_clone = destination.clone();
        let mut consent_service_clone = consent_service.clone();
        tokio::task::spawn(async move {
            assert!(matches!(
                consent_service_clone
                    .call(ConsentRequest::RequestConsent {
                        source: resource_clone,
                        destination: destination_clone,
                    })
                    .await
                    .unwrap(),
                ConsentResponse::Consent(true)
            ));
        });

        assert_eq!(notifications_feed.recv().await.unwrap(), destination.clone());
        consent_service
            .call(ConsentRequest::SetConsent { source: resource, destination, consent: true })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_consent_service_with_ownership_with_decision_timeout() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut consent_service = ConsentService::new(1);
        let resource = Resource::new_process_mock(0);
        let destination =
            Destination::new(None, Some(Resource::new_file("/tmp/test.txt".to_string())));
        let request = ConsentRequest::TakeResourceOwnership(resource.clone());
        let ownership_response = consent_service.call(request).await.unwrap();
        let ConsentResponse::Notifications(mut notifications_feed) = ownership_response else {
            panic!("Expected Notifications");
        };

        // Spawn a task to check the consent request timeout before the decision is sent
        let resource_clone = resource.clone();
        let destination_clone = destination.clone();
        let mut consent_service_clone = consent_service.clone();
        tokio::task::spawn(async move {
            assert!(matches!(
                consent_service_clone
                    .call(ConsentRequest::RequestConsent {
                        source: resource_clone,
                        destination: destination_clone,
                    })
                    .await
                    .unwrap_err(),
                TraceabilityError::ConsentRequestTimeout
            ));
        });

        tokio::time::sleep(Duration::from_millis(2)).await;
        assert_eq!(notifications_feed.recv().await.unwrap(), destination.clone());
        consent_service
            .call(ConsentRequest::SetConsent {
                source: resource.clone(),
                destination: destination.clone(),
                consent: true,
            })
            .await
            .unwrap();

        // Check the consent request timeout after the decision is sent
        assert!(matches!(
            consent_service
                .call(ConsentRequest::RequestConsent { source: resource, destination })
                .await
                .unwrap(),
            ConsentResponse::Consent(true)
        ));
    }
}
