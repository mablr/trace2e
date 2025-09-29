//! Consent service for managing user/operator consent for outgoing data flows.
use std::sync::Arc;

use dashmap::{DashMap, Entry};
use tokio::sync::broadcast;

use crate::traceability::{error::TraceabilityError, naming::Resource};

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
                    rx.recv().await.map_err(|_| TraceabilityError::InternalTrace2eError)
                }
            },
            Entry::Vacant(vac) => {
                let (tx, mut rx) = broadcast::channel(16);
                vac.insert(ConsentState::Pending { tx });
                rx.recv().await.map_err(|_| TraceabilityError::InternalTrace2eError)
            }
        }
    }

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
