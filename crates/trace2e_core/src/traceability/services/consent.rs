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

/// Destination for consent requests with hierarchical specificity.
///
/// Destinations form a hierarchy from most to least specific:
/// - `Resource(Some(node_id), resource)` targets a specific resource on a node
/// - `Node(node_id)` targets all resources on a node
/// - `None` no specific destination (convenience variant)
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Destination {
    /// A specific resource on a node. node_id acn be None when target is a local resource.
    Resource(Option<String>, Resource),
    /// All resources on a specific node.
    Node(String),
    /// No specific destination (convenience variant).
    None,
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

    /// Get hierarchical consent keys from most specific to least specific.
    ///
    /// For a resource destination, returns: [Resource-level, Node-level]
    /// For a node destination, returns: [Node-level]
    fn get_consent_hierarchy(
        &self,
        source: &Resource,
        destination: &Destination,
    ) -> Vec<ConsentKey> {
        match destination {
            Destination::Resource(Some(node_id), resource) => {
                vec![
                    ConsentKey(
                        source.clone(),
                        Destination::Resource(Some(node_id.clone()), resource.clone()),
                    ),
                    ConsentKey(source.clone(), Destination::Node(node_id.clone())),
                ]
            }
            Destination::Resource(None, resource) => {
                vec![ConsentKey(source.clone(), Destination::Resource(None, resource.clone()))]
            }
            Destination::Node(node_id) => {
                vec![ConsentKey(source.clone(), Destination::Node(node_id.clone()))]
            }
            Destination::None => {
                vec![] // No specific destination, so no hierarchy
            }
        }
    }

    /// Check for existing consent decisions in the hierarchy.
    /// Returns the most specific consent decision if found.
    fn check_consent_hierarchy(
        &self,
        source: &Resource,
        destination: &Destination,
    ) -> Option<bool> {
        for key in self.get_consent_hierarchy(source, destination) {
            if let Some(consent) = self.states.get(&key) {
                return Some(*consent);
            }
        }
        None
    }

    /// Internal method to get consent
    async fn get_consent(
        &self,
        source: Resource,
        destination: Destination,
    ) -> Result<bool, TraceabilityError> {
        // Check hierarchy for existing decision (most specific first)
        if let Some(consent) = self.check_consent_hierarchy(&source, &destination) {
            return Ok(consent);
        }

        // No existing decision, proceed with request flow
        let key = ConsentKey(source, destination);
        if let Some(notif_feed) = self.notifications_channels.get(&key.0) {
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

    #[tokio::test]
    async fn test_hierarchical_consent_resource_overrides_node() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut consent_service = ConsentService::new(0);
        let source = Resource::new_process_mock(0);
        let node_id = "node1".to_string();
        let resource = Resource::new_file("/tmp/test.txt".to_string());

        // Set node-level consent to false
        consent_service
            .call(ConsentRequest::SetConsent {
                source: source.clone(),
                destination: Destination::Node(node_id.clone()),
                consent: false,
            })
            .await
            .unwrap();

        // Set resource-level consent to true (should override node-level)
        consent_service
            .call(ConsentRequest::SetConsent {
                source: source.clone(),
                destination: Destination::Resource(Some(node_id.clone()), resource.clone()),
                consent: true,
            })
            .await
            .unwrap();

        // Request consent for the resource - should get true (resource-level overrides node-level)
        let response = consent_service
            .call(ConsentRequest::RequestConsent {
                source: source.clone(),
                destination: Destination::Resource(Some(node_id), resource),
            })
            .await
            .unwrap();

        assert!(matches!(response, ConsentResponse::Consent(true)));
    }

    #[tokio::test]
    async fn test_hierarchical_consent_node_level_fallback() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut consent_service = ConsentService::new(0);
        let source = Resource::new_process_mock(0);
        let node_id = "node1".to_string();
        let resource = Resource::new_file("/tmp/test.txt".to_string());

        // Set only node-level consent to true
        consent_service
            .call(ConsentRequest::SetConsent {
                source: source.clone(),
                destination: Destination::Node(node_id.clone()),
                consent: true,
            })
            .await
            .unwrap();

        // Request consent for a resource on that node - should fall back to node-level
        let response = consent_service
            .call(ConsentRequest::RequestConsent {
                source: source.clone(),
                destination: Destination::Resource(Some(node_id), resource),
            })
            .await
            .unwrap();

        assert!(matches!(response, ConsentResponse::Consent(true)));
    }

    #[tokio::test]
    async fn test_hierarchical_consent_most_specific_wins() {
        #[cfg(feature = "trace2e_tracing")]
        crate::trace2e_tracing::init();
        let mut consent_service = ConsentService::new(0);
        let source = Resource::new_process_mock(0);
        let node_id = "node1".to_string();
        let resource1 = Resource::new_file("/tmp/allowed.txt".to_string());
        let resource2 = Resource::new_file("/tmp/denied.txt".to_string());

        // Set node-level consent to true (permissive default)
        consent_service
            .call(ConsentRequest::SetConsent {
                source: source.clone(),
                destination: Destination::Node(node_id.clone()),
                consent: true,
            })
            .await
            .unwrap();

        // Set resource-level consent to false for specific resource (more specific, should win)
        consent_service
            .call(ConsentRequest::SetConsent {
                source: source.clone(),
                destination: Destination::Resource(Some(node_id.clone()), resource2.clone()),
                consent: false,
            })
            .await
            .unwrap();

        // Resource1 should inherit node-level consent (true)
        let response1 = consent_service
            .call(ConsentRequest::RequestConsent {
                source: source.clone(),
                destination: Destination::Resource(Some(node_id.clone()), resource1),
            })
            .await
            .unwrap();
        assert!(matches!(response1, ConsentResponse::Consent(true)));

        // Resource2 should use its specific consent (false), overriding node-level
        let response2 = consent_service
            .call(ConsentRequest::RequestConsent {
                source,
                destination: Destination::Resource(Some(node_id), resource2),
            })
            .await
            .unwrap();
        assert!(matches!(response2, ConsentResponse::Consent(false)));
    }
}
