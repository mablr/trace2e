///! Traceability server module.
///!
///!
use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};
use tokio::{
    sync::{mpsc, oneshot, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard},
    time::timeout,
};
use tonic::{transport::Channel, Request};
use tracing::{debug, info};

use crate::{
    identifier::Identifier,
    labels::{Compliance, ComplianceSettings, Labels, Provenance},
    m2m_service::m2m::{self, m2m_client::M2mClient},
};

use super::{TraceabilityError, TraceabilityRequest, TraceabilityResponse};

type ContainersMap = HashMap<Identifier, Arc<RwLock<Labels>>>;
type FlowsReleaseHandles = Arc<Mutex<HashMap<u64, oneshot::Sender<()>>>>;

/// The main traceability server structure that holds all the state and functionality
pub struct TraceabilityServer {
    containers: ContainersMap,
    grant_counter: Arc<Mutex<u64>>,
    flows_release_handles: FlowsReleaseHandles,
}

impl TraceabilityServer {
    /// Create a new traceability server instance
    pub fn new() -> Self {
        Self {
            containers: HashMap::new(),
            grant_counter: Arc::new(Mutex::new(0)),
            flows_release_handles: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Start the server and process incoming requests
    pub async fn run(mut self, mut receiver: mpsc::Receiver<TraceabilityRequest>) {
        while let Some(message) = receiver.recv().await {
            match message {
                TraceabilityRequest::RegisterContainer(identifier, responder) => {
                    self.handle_register_container(identifier, responder).await;
                }
                TraceabilityRequest::SetComplianceLabel(
                    identifier,
                    local_confidentiality,
                    local_integrity,
                    responder,
                ) => {
                    self.set_compliance_label(
                        identifier,
                        local_confidentiality,
                        local_integrity,
                        responder,
                    )
                    .await;
                }
                TraceabilityRequest::DeclareFlow(id1, id2, output, responder) => {
                    match self.validate_flow(id1.clone(), id2.clone()) {
                        Ok((pid, id1_container, id2_container)) => {
                            let flows_release_handles = Arc::clone(&self.flows_release_handles);
                            let grant_counter = Arc::clone(&self.grant_counter);
                            tokio::spawn(async move {
                                let server = TraceabilityServer {
                                    containers: HashMap::new(),
                                    grant_counter,
                                    flows_release_handles,
                                };
                                server.handle_flow(
                                    id1,
                                    id1_container,
                                    id2,
                                    id2_container,
                                    pid,
                                    output,
                                    responder,
                                )
                                .await;
                            });
                        }
                        Err(e) => responder.send(TraceabilityResponse::Error(e)).unwrap(),
                    }
                }
                TraceabilityRequest::RecordFlow(grant_id, responder) => {
                    self.handle_record_flow(grant_id, responder).await;
                }
                TraceabilityRequest::SyncStream(identifier, responder) => {
                    self.handle_sync_stream(identifier, responder).await;
                }
                TraceabilityRequest::PrintProvenance => {
                    self.print_provenance().await;
                }
            }
        }
    }

    /// Register a new container
    async fn handle_register_container(
        &mut self,
        identifier: Identifier,
        responder: oneshot::Sender<TraceabilityResponse>,
    ) {
        if let Some(container) = self.containers.get(&identifier).cloned() {
            if identifier.is_stream().is_some() {
                container.write().await.clear_prov(); // Purge previous provenance
            }
        } else {
            self.containers.insert(
                identifier.clone(),
                Arc::new(RwLock::new(Labels::new(identifier.clone()))),
            );
        }
        responder
            .send(TraceabilityResponse::Registered(identifier))
            .unwrap();
    }

    /// Set compliance label for a container
    async fn set_compliance_label(
        &mut self,
        identifier: Identifier,
        local_confidentiality: Option<bool>,
        local_integrity: Option<bool>,
        responder: oneshot::Sender<TraceabilityResponse>,
    ) {
        if let Some(container) = self.containers.get(&identifier).cloned() {
            let mut labels = container.write().await;
            self.set_local_compliance_rules(&mut labels, local_confidentiality, local_integrity);
        } else {
            let mut labels = Labels::new(identifier.clone());
            self.set_local_compliance_rules(&mut labels, local_confidentiality, local_integrity);
            self.containers.insert(identifier.clone(), Arc::new(RwLock::new(labels)));
        }
        responder
            .send(TraceabilityResponse::Registered(identifier))
            .unwrap();
    }

    /// Set local compliance rules for labels
    fn set_local_compliance_rules(
        &self,
        labels: &mut Labels,
        local_confidentiality: Option<bool>,
        local_integrity: Option<bool>,
    ) {
        if let Some(value) = local_confidentiality {
            labels.set_local_confidentiality(value);
        }
        if let Some(value) = local_integrity {
            labels.set_local_integrity(value);
        }
    }

    /// Handle a flow between containers
    async fn handle_flow(
        &self,
        id1: Identifier,
        id1_container: Arc<RwLock<Labels>>,
        id2: Identifier,
        id2_container: Arc<RwLock<Labels>>,
        pid: u32,
        output: bool,
        responder: oneshot::Sender<TraceabilityResponse>,
    ) {
        if let Some((local_socket, peer_socket)) = id2.is_stream().filter(|_| output) {
            // Output Flow to stream (Decentralized procedure)
            match self.reserve_remote_flow(&id1_container, local_socket, peer_socket).await {
                Ok((source_labels, destination_labels, mut client)) => {
                    if destination_labels.is_compliant(&source_labels) {
                        let (grant_id, release) = self.grant_flow(responder).await;
                        debug!(
                            "[TS] ✅ Flow {} granted ({:?} {} {:?})",
                            grant_id,
                            id1.clone(),
                            if output { "->" } else { "<-" },
                            id2.clone()
                        );

                        match timeout(Duration::from_millis(50), release).await {
                            Err(_) | Ok(Err(_)) => {
                                debug!("[TS] ⚠️  Reservation timeout Flow {}", grant_id);

                                // Try to kill the process that holds the reservation for too long,
                                // to release the reservation safely
                                let _ = std::process::Command::new("kill")
                                    .arg("-9")
                                    .arg(pid.to_string())
                                    .status();

                                // Release remote stream
                                let _ = client
                                    .sync_provenance(Request::new(m2m::StreamProv {
                                        local_socket: local_socket.to_string(),
                                        peer_socket: peer_socket.to_string(),
                                        provenance: Vec::new(), // empty provenance to skip update and release remote stream
                                    }))
                                    .await;

                                // Remove flow release handle
                                self.flows_release_handles.lock().await.remove(&grant_id);
                            }
                            Ok(Ok(_)) => {
                                debug!(
                                    "[TS] 🔼 Flow {} Sync to {} ({:?} -> {:?})",
                                    grant_id,
                                    peer_socket.ip(),
                                    id1.clone(),
                                    id2.clone()
                                );
                                info!(
                                    "[M2M] LM-> sync_prov (Stream: [{}-{}])",
                                    local_socket, peer_socket
                                );
                                let _ = client
                                    .sync_provenance(Request::new(m2m::StreamProv {
                                        local_socket: local_socket.to_string(),
                                        peer_socket: peer_socket.to_string(),
                                        provenance: source_labels
                                            .get_all_labels()
                                            .into_iter()
                                            .map(m2m::ComplianceLabel::from)
                                            .collect(),
                                    }))
                                    .await; // Todo Sync failure management
                                info!(
                                    "[M2M] LM<- sync_prov (Stream: [{}-{}])",
                                    local_socket, peer_socket
                                );
                            }
                        }
                        debug!("[TS] 🗑️  Flow {} destruction", grant_id);
                    } else {
                        debug!(
                            "[TS] ⛔ Flow refused ({:?} {} {:?})",
                            id1.clone(),
                            if output { "->" } else { "<-" },
                            id2.clone()
                        );

                        // Release remote stream
                        let _ = client
                            .sync_provenance(Request::new(m2m::StreamProv {
                                local_socket: local_socket.to_string(),
                                peer_socket: peer_socket.to_string(),
                                provenance: Vec::new(), // empty provenance to skip update and release remote stream
                            }))
                            .await;

                        responder
                            .send(TraceabilityResponse::Error(
                                TraceabilityError::ForbiddenFlow(id1, id2),
                            ))
                            .unwrap();
                    }
                }
                Err(e) => responder.send(TraceabilityResponse::Error(e)).unwrap(),
            }
        } else {
            // File IO Flow, or Input flow from Stream (Local procedure)
            let (source_labels, mut destination_labels) =
                self.reserve_local_flow(output, &id1_container, &id2_container).await;

            if destination_labels.is_compliant(&source_labels) {
                let (grant_id, release) = self.grant_flow(responder).await;
                debug!(
                    "[TS] ✅ Flow {} granted ({:?} {} {:?})",
                    grant_id,
                    id1.clone(),
                    if output { "->" } else { "<-" },
                    id2.clone()
                );

                match timeout(Duration::from_millis(50), release).await {
                    Err(_) | Ok(Err(_)) => {
                        debug!("[TS] ⚠️  Reservation timeout Flow {}", grant_id);

                        // Try to kill the process that holds the reservation for too long,
                        // to release the reservation safely
                        let _ = std::process::Command::new("kill")
                            .arg("-9")
                            .arg(pid.to_string())
                            .status();

                        // Remove flow release handle
                        self.flows_release_handles.lock().await.remove(&grant_id);
                    }
                    Ok(Ok(_)) => {
                        // Record flow locally if it is not an output to a stream
                        debug!(
                            "[TS] ⏺️  Flow {} recording ({:?} {} {:?})",
                            grant_id,
                            id1.clone(),
                            if output { "->" } else { "<-" },
                            id2.clone()
                        );
                        destination_labels.update_prov(&source_labels);
                        debug!(
                            "[TS] 🆕 Provenance: {{{:?}: {:?},  {:?}: {:?}}}",
                            if output { id1.clone() } else { id2.clone() },
                            source_labels.get_prov(),
                            if output { id2.clone() } else { id1.clone() },
                            destination_labels.get_prov(),
                        );
                    }
                }
                debug!("[TS] 🗑️  Flow {} destruction", grant_id);
            } else {
                debug!(
                    "[TS] ⛔ Flow refused ({:?} {} {:?})",
                    id1.clone(),
                    if output { "->" } else { "<-" },
                    id2.clone()
                );
                responder
                    .send(TraceabilityResponse::Error(
                        TraceabilityError::ForbiddenFlow(id1, id2),
                    ))
                    .unwrap();
            }
        }
    }

    /// Validate a flow between containers
    fn validate_flow(
        &self,
        id1: Identifier,
        id2: Identifier,
    ) -> Result<(u32, Arc<RwLock<Labels>>, Arc<RwLock<Labels>>), TraceabilityError> {
        if let Some(pid) = id1.is_process().filter(|_| id2.is_process().is_none()) {
            let id1_container = self.containers.get(&id1).cloned();
            let id2_container = self.containers.get(&id2).cloned();
            if let (Some(id1_container), Some(id2_container)) =
                (id1_container.clone(), id2_container.clone())
            {
                Ok((pid, id1_container, id2_container))
            } else {
                Err(TraceabilityError::MissingRegistration(
                    if id1_container.is_none() {
                        Some(id1)
                    } else {
                        None
                    },
                    if id2_container.is_none() {
                        Some(id2)
                    } else {
                        None
                    },
                ))
            }
        } else {
            Err(TraceabilityError::InvalidFlow(id1, id2))
        }
    }

    /// Reserve a local flow between containers
    async fn reserve_local_flow<'a>(
        &self,
        output: bool,
        id1_container: &'a Arc<RwLock<Labels>>,
        id2_container: &'a Arc<RwLock<Labels>>,
    ) -> (RwLockReadGuard<'a, Labels>, RwLockWriteGuard<'a, Labels>) {
        if output {
            let rguard = id1_container.read().await;
            let wguard = id2_container.write().await;
            (rguard, wguard)
        } else {
            let wguard = id1_container.write().await;
            let rguard = id2_container.read().await;
            (rguard, wguard)
        }
    }

    /// Reserve a remote flow between containers
    async fn reserve_remote_flow<'a>(
        &self,
        id_container: &'a Arc<RwLock<Labels>>,
        local_socket: SocketAddr,
        peer_socket: SocketAddr,
    ) -> Result<(RwLockReadGuard<'a, Labels>, Labels, M2mClient<Channel>), TraceabilityError> {
        info!(
            "[M2M] LM-> reserve (Stream: [{}-{}])",
            local_socket, peer_socket
        );
        if let Ok(mut client) = M2mClient::connect(format!("http://{}:8080", peer_socket.ip())).await {
            let source_labels = id_container.read().await;
            match client
                .reserve(Request::new(m2m::Stream {
                    local_socket: local_socket.to_string(),
                    peer_socket: peer_socket.to_string(),
                }))
                .await
            {
                Ok(response) => {
                    info!(
                        "[M2M] LM<- reserve (Stream: [{}-{}])",
                        local_socket, peer_socket
                    );
                    Ok((source_labels, response.into_inner().into(), client))
                }
                Err(_) => Err(TraceabilityError::MissingRegistrationRemote(
                    peer_socket,
                    local_socket,
                )),
            }
        } else {
            Err(TraceabilityError::NonCompliantRemote(peer_socket))
        }
    }

    /// Grant a flow and return the grant ID and release channel
    async fn grant_flow(
        &self,
        responder: oneshot::Sender<TraceabilityResponse>,
    ) -> (u64, oneshot::Receiver<()>) {
        let grant_id = {
            let mut grant_counter = self.grant_counter.lock().await;
            *grant_counter += 1;
            *grant_counter
        };
        let (release_callback, release) = oneshot::channel();
        self.flows_release_handles
            .lock()
            .await
            .insert(grant_id, release_callback);

        responder
            .send(TraceabilityResponse::Declared(grant_id))
            .unwrap();

        (grant_id, release)
    }

    /// Handle syncing a stream
    async fn handle_sync_stream(
        &self,
        identifier: Identifier,
        responder: oneshot::Sender<TraceabilityResponse>,
    ) {
        if let Some(id_container) = identifier
            .is_stream()
            .and(self.containers.get(&identifier))
            .cloned()
        {
            tokio::spawn(async move {
                let mut stream_labels = id_container.write().await;
                let (provenance_sender, provenance_receiver) = oneshot::channel();
                responder
                    .send(TraceabilityResponse::WaitingSync(
                        stream_labels.clone(),
                        provenance_sender,
                    ))
                    .unwrap();

                if let Ok((provenance, callback)) = provenance_receiver.await {
                    stream_labels.set_prov(provenance);
                    let _ = callback.send(TraceabilityResponse::Recorded);
                    debug!("[TS] 🔽 Remote provenance sync on {:?}", identifier.clone());
                    debug!(
                        "[TS] 🆕 Provenance: {{{:?}: {:?}}}",
                        identifier.clone(),
                        stream_labels.get_prov()
                    );
                } else {
                    debug!(
                        "[TS] ⛔ Remote provenance sync on {:?} failed",
                        identifier.clone()
                    );

                    // Todo handle provenance sync failures
                    todo!();
                }
            });
        } else {
            responder
                .send(TraceabilityResponse::Error(
                    TraceabilityError::MissingRegistrationStream(identifier),
                ))
                .unwrap();
        }
    }

    /// Handle recording a flow
    async fn handle_record_flow(
        &self,
        grant_id: u64,
        responder: oneshot::Sender<TraceabilityResponse>,
    ) {
        if let Some(release_handle) = self.flows_release_handles.lock().await.remove(&grant_id) {
            release_handle.send(()).unwrap();
            // No wait for release feedback, the consistency is guaranted by RwLock<> structure properties.
            responder.send(TraceabilityResponse::Recorded).unwrap();
        } else {
            responder
                .send(TraceabilityResponse::Error(
                    TraceabilityError::RecordingFailure(grant_id),
                ))
                .unwrap();
        }
    }

    /// Print the current provenance state
    async fn print_provenance(&self) {
        println!("[");
        for (id, label) in &self.containers {
            if id.is_stream().is_none() {
                let label = label.read().await;
                println!(
                    "  {{ \"{}\" (local_confidentiality: {}, local_integrity: {}):\n    [",
                    id,
                    label.get_local_confidentiality(),
                    label.get_local_integrity()
                );
                for cl in label.get_prov() {
                    println!("      \"{:?}\",", cl);
                }
                println!("    ]\n  }},");
            }
        }
        println!("]");
    }
}

/// Create and run a new traceability server
pub fn init_traceability_server() -> mpsc::Sender<TraceabilityRequest> {
    let server = TraceabilityServer::new();
    let (sender, receiver) = mpsc::channel(100);
    tokio::spawn(async move {
        server.run(receiver).await;
    });

    sender
}
