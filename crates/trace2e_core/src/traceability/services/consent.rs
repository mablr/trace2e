//! Consent service for managing consent for outgoing data flows of resources.
use std::{future::Future, pin::Pin, sync::Arc, task::Poll};

use dashmap::{DashMap, Entry};
use tokio::{sync::broadcast, time::Duration};
use tower::Service;
use tracing::info;

use crate::traceability::{
    error::TraceabilityError,
    infrastructure::naming::{LocalizedResource, Resource},
};

/// Consent service request types.
///
/// API for the consent service, which manages user consent for data flow operations.
#[derive(Debug, PartialEq)]
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

/// Destination for consent requests with built-in hierarchical structure.
///
/// The hierarchy is encoded in the type itself:
/// - A `Resource` can have a `parent` destination (typically a `Node`)
/// - This creates a natural traversal from specific to broad scopes
///
/// Example: `Resource { resource: file, parent: Some(Node("node1")) }`
/// represents a file on node1, with node1 as the fallback consent scope.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Destination {
    /// A specific resource, optionally with a parent destination for hierarchy.
    Resource { resource: Resource, parent: Option<Box<Self>> },
    /// A node destination.
    Node(String),
}

impl From<LocalizedResource> for Destination {
    fn from(localized_resource: LocalizedResource) -> Self {
        Self::Resource {
            resource: localized_resource.resource().to_owned(),
            parent: Some(Box::new(Self::Node(localized_resource.node_id().to_owned()))),
        }
    }
}

/// Parse a Destination from a string.
/// Tries three formats in order:
/// 1. LocalizedResource: "node_id@resource_spec"
/// 2. Resource: "file:///path" or "stream://local::peer"
/// 3. Node ID: a simple string without whitespaces
impl TryFrom<&str> for Destination {
    type Error = String;

    fn try_from(s: &str) -> Result<Destination, Self::Error> {
        let s = s.trim();

        // Try parsing as LocalizedResource first (contains '@')
        if s.contains('@')
            && let Ok(lr) = LocalizedResource::try_from(s)
        {
            return Ok(Destination::Resource {
                resource: lr.resource().clone(),
                parent: Some(Box::new(Destination::Node(lr.node_id().clone()))),
            });
        }

        // Try parsing as Resource (contains '://' or starts with specific patterns)
        if s.contains("://")
            && let Ok(resource) = Resource::try_from(s)
        {
            return Ok(Destination::Resource { resource, parent: None });
        }

        // Try as node ID (simple string without whitespaces)
        if !s.contains(char::is_whitespace) && !s.is_empty() {
            return Ok(Destination::Node(s.to_string()));
        }

        Err(format!(
            "Failed to parse destination: '{}'. Expected one of: node_id, resource (file:// or stream://), or localized_resource (node_id@resource)",
            s
        ))
    }
}

impl TryFrom<String> for Destination {
    type Error = String;

    fn try_from(s: String) -> Result<Destination, Self::Error> {
        Destination::try_from(s.as_str())
    }
}

impl Destination {
    /// Create a new destination from optional node_id and resource.
    pub fn new(node_id: Option<String>, resource: Option<Resource>) -> Self {
        match (node_id, resource) {
            (Some(node_id), Some(resource)) => {
                Self::Resource { resource, parent: Some(Box::new(Self::Node(node_id))) }
            }
            (None, Some(resource)) => Self::Resource { resource, parent: None },
            (Some(node_id), None) => Self::Node(node_id),
            (None, None) => panic!("Cannot create Destination with no node_id and no resource"),
        }
    }

    /// Iterator over the hierarchy from most specific to least specific.
    ///
    /// Example hierarchy traversal:
    /// ```text
    /// Resource { resource: file, parent: Some(Node("node1")) }
    ///   -> yields: Resource(file), Node("node1")
    /// ```
    fn hierarchy(&self) -> impl Iterator<Item = &Destination> {
        std::iter::successors(Some(self), |dest| match dest {
            Destination::Resource { parent, .. } => parent.as_deref(),
            Destination::Node(_) => None,
        })
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

    /// Check for existing consent decisions in the hierarchy.
    /// Returns the most specific consent decision if found.
    ///
    /// Traverses from most specific (resource) to least specific (node).
    fn check_consent_hierarchy(
        &self,
        source: &Resource,
        destination: &Destination,
    ) -> Option<bool> {
        destination.hierarchy().find_map(|dest| {
            let key = ConsentKey(source.clone(), dest.clone());
            self.states.get(&key).map(|v| *v)
        })
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

        // CRITICAL: Clone the broadcast sender OUTSIDE the DashMap guard
        // to avoid holding a lock during async recv() operations
        let notif_sender = match self.notifications_channels.get(&key.0) {
            Some(sender_ref) => sender_ref.clone(),
            None => {
                // No notifications feed, so nobody will ever know about this consent request
                // and it will never be granted
                return Ok(false);
            }
        };
        // The sender is now cloned and the guard is dropped above

        // Send consent request notification and get decision receiver
        // CRITICAL: Clone the decision sender OUTSIDE the DashMap guard to avoid lock issues
        let decision_sender = self.decision_channels.get(&key).map(|feed| feed.clone());
        // Guard is now dropped

        let mut decision_rx = if let Some(decision_sender) = decision_sender {
            // Existing decision channel - subscribe to it
            notif_sender
                .send(key.1.clone())
                .map_err(|_| TraceabilityError::InternalTrace2eError)?;
            decision_sender.subscribe() // ✅ No DashMap lock held
        } else {
            // First request for this source→destination - create new decision channel
            let (tx, rx) = broadcast::channel::<bool>(100);
            self.decision_channels.insert(key.clone(), tx);
            notif_sender.send(key.1).map_err(|_| TraceabilityError::InternalTrace2eError)?;
            rx
        };

        // Handle timeout - no DashMap locks held here
        if self.timeout > 0 {
            tokio::time::timeout(Duration::from_millis(self.timeout), decision_rx.recv())
                .await
                .map_err(|_| TraceabilityError::ConsentRequestTimeout)?
                .map_err(|_| TraceabilityError::InternalTrace2eError)
        } else {
            decision_rx.recv().await.map_err(|_| TraceabilityError::InternalTrace2eError)
        }
    }

    /// Internal method to set consent
    fn set_consent(&self, source: Resource, destination: Destination, consent: bool) {
        let key = ConsentKey(source.clone(), destination.clone());
        // Insert the consent decision into the persistent state
        self.states.insert(key.clone(), consent);

        // Always attempt to notify any waiting decision receivers
        // This includes:
        // 1. Exact key match (specific resource request)
        // 2. Hierarchical matches (parent destinations like nodes)

        // First, try exact match
        if let Some((_, decision_feed)) = self.decision_channels.remove(&key) {
            let _ = decision_feed.send(consent);
        }

        // Then, try to notify any child destinations that might be waiting
        // For example, if we set consent at Node level, notify all Resource-level requests
        // with that node as parent
        let mut keys_to_remove = Vec::new();
        for entry in self.decision_channels.iter() {
            let other_key = entry.key();
            // Check if this other_key matches our hierarchy
            // It matches if: same source AND other_key's destination has our destination as parent
            if other_key.0 == source {
                // Check if setting destination satisfies the hierarchy requirement for other_key
                let other_dest = &other_key.1;

                // Build the hierarchy of other_dest and check if our destination is in it
                let mut current: Option<&Destination> = Some(other_dest);
                while let Some(dest) = current {
                    if dest == &destination {
                        // Found a match! This waiting request should be notified
                        keys_to_remove.push(other_key.clone());
                        break;
                    }
                    // Move to parent in hierarchy
                    current = match dest {
                        Destination::Resource { parent, .. } => parent.as_deref(),
                        Destination::Node(_) => None,
                    };
                }
            }
        }

        // Remove and notify all matching keys
        for key_to_remove in keys_to_remove {
            if let Some((_, decision_feed)) = self.decision_channels.remove(&key_to_remove) {
                let _ = decision_feed.send(consent);
            }
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
                    info!(source = %source, destination = ?destination, "[consent] RequestConsent");
                    this.get_consent(source, destination).await.map(ConsentResponse::Consent)
                }
                ConsentRequest::SetConsent { source, destination, consent } => {
                    info!(consent = %consent, source = %source, destination = ?destination, "[consent] SetConsent");
                    this.set_consent(source, destination, consent);
                    Ok(ConsentResponse::Ack)
                }
                ConsentRequest::TakeResourceOwnership(resource) => {
                    info!(resource = %resource, "[consent] TakeResourceOwnership");
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
                destination: Destination::new(Some(node_id.clone()), Some(resource.clone())),
                consent: true,
            })
            .await
            .unwrap();

        // Request consent for the resource - should get true (resource-level overrides node-level)
        let response = consent_service
            .call(ConsentRequest::RequestConsent {
                source: source.clone(),
                destination: Destination::new(Some(node_id), Some(resource)),
            })
            .await
            .unwrap();

        assert!(matches!(response, ConsentResponse::Consent(true)));
    }

    #[tokio::test]
    async fn test_hierarchical_consent_node_level_fallback() {
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
                destination: Destination::new(Some(node_id), Some(resource)),
            })
            .await
            .unwrap();

        assert!(matches!(response, ConsentResponse::Consent(true)));
    }

    #[tokio::test]
    async fn test_hierarchical_consent_most_specific_wins() {
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
                destination: Destination::new(Some(node_id.clone()), Some(resource2.clone())),
                consent: false,
            })
            .await
            .unwrap();

        // Resource1 should inherit node-level consent (true)
        let response1 = consent_service
            .call(ConsentRequest::RequestConsent {
                source: source.clone(),
                destination: Destination::new(Some(node_id.clone()), Some(resource1)),
            })
            .await
            .unwrap();
        assert!(matches!(response1, ConsentResponse::Consent(true)));

        // Resource2 should use its specific consent (false), overriding node-level
        let response2 = consent_service
            .call(ConsentRequest::RequestConsent {
                source,
                destination: Destination::new(Some(node_id), Some(resource2)),
            })
            .await
            .unwrap();
        assert!(matches!(response2, ConsentResponse::Consent(false)));
    }
}
