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

/// This asynchronous function is responsible of traceability tracking, it
/// provides a global consistent provenance state in a fully asynchronous
/// paradigm.
///
/// To do so, it listens to a [`tokio`] mpsc channel for incoming
/// [`TraceabilityRequest`] messages and processes them accordingly.
///
/// It manages the registration of containers, declaration of flows between
/// registered containers, and recording of flows.
///
/// The function is spawned once to hold the global provenance state of all
/// containers, by associating each registered [`Identifier`] to a [`Labels`]
/// object wrapped in a [`RwLock`][tokio::sync::RwLock] (see [Flow reservation
/// mechanism](#flow-reservation-mechanism) for more details).
///
/// # Parameters
///
/// - `receiver`: A mutable [`mpsc::Receiver`] that listens for incoming
/// [`TraceabilityRequest`] messages.
///
/// # Behavior
///
/// The function handles 3 different actions matched with the variants of the
/// [`TraceabilityRequest`] enum:
///
/// 1. [`TraceabilityRequest::RegisterContainer`]:
///    - Registers a new container identified by a unique [`Identifier`] object
///     if it is a File or a Process and not already registered (overwritting
///     forbidden), else if it is a Stream (overwritting allowed).
///    - Sends a `TraceabilityResponse::Registered` back through the responder once
///     the registration is successful.
///
/// 2. [`TraceabilityRequest::DeclareFlow`]:
///    - Validates the flow : checks if the involved containers are registered,
///     and if the containers types are valid.
///    - If the flow is valid, it spawns an asynchronous task to manage the flow
///     reservation (see [Flow reservation
///     mechanism](#flow-reservation-mechanism) for more details).
///
/// 3. [`TraceabilityRequest::RecordFlow`]:
///    - If the grant ID is found, it sends a signal to the task that holds the
///     corresponding flow reservation to record the flow in the provenance,
///     then to drop then the reservation of the containers and it returns a
///     [`TraceabilityResponse::Recorded`] message.
///    - Otherwise, it returns a [`TraceabilityResponse`] containing to following
///     error [`TraceabilityError::RecordingFailure`].
///
/// # Flow reservation mechanism
///
/// To enable asynchronous tracking of the origin of data containers, it is
/// necessary to establish a system for locking containers during a flow.
///
/// A data flow is established by identifying a pair of containers and given an
/// in/out direction. The pair of containers consist of a single process
/// container, since a flow, whether incoming or outgoing, is always carried on
/// by a process, with its peer container which is not a process.
///
/// During a data flow, the source container is read and the destination
/// container is written. The traceability server propagates the provenance
/// information by locking the corresponding pair of `Labels` objects.
///
/// By wrapping each `Labels` object in a [`RwLock`][tokio::sync::RwLock],
/// coherence is guaranteed in an asynchronous context.
///
/// Once the flow has been successsfully checked (containers registration and
/// types), a Tokio task is spawned to reserve the wrapped Labels objects in the
/// appropriate mode. Once the reservations have been obtained through RwLock
/// primitives, a RwLockReadGuard object and a RwLockWriteGuard object are
/// instantiated to hold the reservations.
///
/// The flow is authorized, and a grant ID is generated. A oneshot channel is
/// then instantiated to receive confirmation of the flow's execution at a later
/// date, in order to proceed with the actual propagation of labels. The sender
/// object of the oneshot channel is stored in a hashmap with the newly
/// generated grant ID as key.
///
/// A message `TraceabilityResponse::Declared` with a unique grant ID confirming the
/// reservation is sent, and the task waits for the flow confirmation message on
/// the oneshot channel.
///
/// This wait for a response is limited by a timeout.
/// - If confirmation arrives on time, the `Labels` object corresponding to the
///     destination container is updated with the `Labels` object associated
///     with the source container.
/// - If the timeout is exceeded, the reservation must be forcibly released, and
///     the process associated with the blocking flow is killed using its PID.
///
/// The task ends, causing the context to exit, destroying the RwLock Guard
/// objects, and the reservation is released.
///
///
/// # Example Usage
///
/// This function is designed to be spawned once, typically within a Tokio
/// runtime. It continuously listens for [`TraceabilityRequest`] messages and
/// processes them based on their variant.
///
/// ```rust
/// use tokio::sync::mpsc;
///
/// #[tokio::main]
/// async fn main() {
///     let (sender, receiver) = mpsc::channel(32);
///
///     // Spawn the traceability server to handle messages
///     tokio::spawn(async move {
///         traceability_server(receiver).await;
///     });
///
///     // Example sending message to the traceability server
///     let (tx, rx) = oneshot::channel();
///     sender.send(TraceabilityRequest::RegisterContainer(Identifier::File("/path/to/file"), rx)).await.unwrap();
///     rx.await.unwrap();
/// }
/// ```
///
/// # Notes
/// This function provides verbose logging (enabled with "verbose" feature flag)
/// for a live view of the traceability server operations.
///
/// [`tokio`]: https://docs.rs/tokio
pub async fn traceability_server(mut receiver: mpsc::Receiver<TraceabilityRequest>) {
    let mut containers: ContainersMap = HashMap::new();
    let grant_counter: Arc<Mutex<u64>> = Arc::new(Mutex::new(0));
    let flows_release_handles: FlowsReleaseHandles = Arc::new(Mutex::new(HashMap::new()));

    while let Some(message) = receiver.recv().await {
        match message {
            TraceabilityRequest::RegisterContainer(identifier, responder) => {
                handle_register_container(&mut containers, identifier, responder).await
            }
            TraceabilityRequest::SetComplianceLabel(
                identifier,
                local_confidentiality,
                local_integrity,
                responder,
            ) => {
                set_compliance_label(
                    &mut containers,
                    identifier,
                    local_confidentiality,
                    local_integrity,
                    responder,
                )
                .await
            }
            TraceabilityRequest::DeclareFlow(id1, id2, output, responder) => {
                match validate_flow(id1.clone(), id2.clone(), &containers) {
                    Ok((pid, id1_container, id2_container)) => {
                        tokio::spawn(handle_flow(
                            id1,
                            id1_container,
                            id2,
                            id2_container,
                            pid,
                            Arc::clone(&grant_counter),
                            Arc::clone(&flows_release_handles),
                            output,
                            responder,
                        ));
                    }
                    Err(e) => responder.send(TraceabilityResponse::Error(e)).unwrap(),
                }
            }
            TraceabilityRequest::RecordFlow(grant_id, responder) => {
                handle_record_flow(grant_id, responder, Arc::clone(&flows_release_handles)).await;
            }
            TraceabilityRequest::SyncStream(identifier, responder) => {
                handle_sync_stream(&containers, identifier, responder).await;
            }
            TraceabilityRequest::PrintProvenance => {
                println!("[");
                for (id, label) in &containers {
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
    }
}

async fn handle_register_container(
    containers: &mut ContainersMap,
    identifier: Identifier,
    responder: oneshot::Sender<TraceabilityResponse>,
) {
    if let Some(container) = containers.get(&identifier).cloned() {
        if identifier.is_stream().is_some() {
            container.write().await.clear_prov(); // Purge previous provenance
        }
    } else {
        containers.insert(
            identifier.clone(),
            Arc::new(RwLock::new(Labels::new(identifier.clone()))),
        );
    }
    responder
        .send(TraceabilityResponse::Registered(identifier))
        .unwrap();
}

async fn set_compliance_label(
    containers: &mut ContainersMap,
    identifier: Identifier,
    local_confidentiality: Option<bool>,
    local_integrity: Option<bool>,
    responder: oneshot::Sender<TraceabilityResponse>,
) {
    if let Some(container) = containers.get(&identifier).cloned() {
        let mut labels = container.write().await;
        set_local_compliance_rules(&mut labels, local_confidentiality, local_integrity);
    } else {
        let mut labels = Labels::new(identifier.clone());
        set_local_compliance_rules(&mut labels, local_confidentiality, local_integrity);
        containers.insert(identifier.clone(), Arc::new(RwLock::new(labels)));
    }
    responder
        .send(TraceabilityResponse::Registered(identifier))
        .unwrap();
}

fn set_local_compliance_rules(
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

async fn handle_flow(
    id1: Identifier,
    id1_container: Arc<RwLock<Labels>>,
    id2: Identifier,
    id2_container: Arc<RwLock<Labels>>,
    pid: u32,
    grant_counter: Arc<Mutex<u64>>,
    flows_release_handles: FlowsReleaseHandles,
    output: bool,
    responder: oneshot::Sender<TraceabilityResponse>,
) {
    if let Some((local_socket, peer_socket)) = id2.is_stream().filter(|_| output) {
        // Output Flow to stream (Decentralized procedure)
        match reserve_remote_flow(&id1_container, local_socket, peer_socket).await {
            Ok((source_labels, destination_labels, mut client)) => {
                if destination_labels.is_compliant(&source_labels) {
                    let (grant_id, release) =
                        grant_flow(grant_counter, flows_release_handles.clone(), responder).await;
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
                            // Todo make specific rpc
                            let _ = client
                                .sync_provenance(Request::new(m2m::StreamProv {
                                    local_socket: local_socket.to_string(),
                                    peer_socket: peer_socket.to_string(),
                                    provenance: Vec::new(), // empty provenance to skip update and release remote stream
                                }))
                                .await;

                            // Remove flow release handle
                            flows_release_handles.lock().await.remove(&grant_id);
                        },
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
                    // Todo: refactor logic sequence
                    debug!(
                        "[TS] ⛔ Flow refused ({:?} {} {:?})",
                        id1.clone(),
                        if output { "->" } else { "<-" },
                        id2.clone()
                    );

                    // Release remote stream
                    // Todo make specific rpc
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
            reserve_local_flow(output, &id1_container, &id2_container).await;

        if destination_labels.is_compliant(&source_labels) {
            let (grant_id, release) =
                grant_flow(grant_counter, flows_release_handles.clone(), responder).await;
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
                    flows_release_handles.lock().await.remove(&grant_id);
                },
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

/// This function validates the flow by checking if it involves one process and
/// one non-process and if the ressources were previously registered.
fn validate_flow(
    id1: Identifier,
    id2: Identifier,
    containers: &ContainersMap,
) -> Result<(u32, Arc<RwLock<Labels>>, Arc<RwLock<Labels>>), TraceabilityError> {
    if let Some(pid) = id1.is_process().filter(|_| id2.is_process().is_none()) {
        let id1_container = containers.get(&id1).cloned();
        let id2_container = containers.get(&id2).cloned();
        if let (Some(id1_container), Some(id2_container)) =
            (id1_container.clone(), id2_container.clone())
        {
            Ok((pid, id1_container, id2_container))
        } else {
            if id1_container.is_some() {
                Err(TraceabilityError::MissingRegistration(id2))
            } else if id2_container.is_some() {
                Err(TraceabilityError::MissingRegistration(id1))
            } else {
                Err(TraceabilityError::MissingRegistrations(id1, id2))
            }
        }
    } else {
        Err(TraceabilityError::InvalidFlow(id1, id2))
    }
}

/// This function acquires the locks with the appropriate R/W mode and returns
/// the guards to guarantee the flow recording consistency.
async fn reserve_local_flow<'a>(
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

/// This function acquires the locks with the appropriate R/W mode and returns
/// the guards to guarantee the flow recording consistency.
async fn reserve_remote_flow<'a>(
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

async fn grant_flow(
    grant_counter: Arc<Mutex<u64>>,
    flows_release_handles: FlowsReleaseHandles,
    responder: oneshot::Sender<TraceabilityResponse>,
) -> (u64, oneshot::Receiver<()>) {
    let grant_id = {
        let mut grant_counter = grant_counter.lock().await;
        *grant_counter += 1;
        *grant_counter
    };
    let (release_callback, release) = oneshot::channel();
    flows_release_handles
        .lock()
        .await
        .insert(grant_id, release_callback);

    responder
        .send(TraceabilityResponse::Declared(grant_id))
        .unwrap();

    (grant_id, release)
}

async fn handle_sync_stream(
    containers: &ContainersMap,
    identifier: Identifier,
    responder: oneshot::Sender<TraceabilityResponse>,
) {
    if let Some(id_container) = identifier
        .is_stream()
        .and(containers.get(&identifier))
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

async fn handle_record_flow(
    grant_id: u64,
    responder: oneshot::Sender<TraceabilityResponse>,
    flows_release_handles: FlowsReleaseHandles,
) {
    if let Some(release_handle) = flows_release_handles.lock().await.remove(&grant_id) {
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

/// Create and run a new traceability server
pub fn spawn_traceability_server() -> mpsc::Sender<TraceabilityRequest> {
    let (sender, receiver) = mpsc::channel(100);
    tokio::spawn(async move {
        traceability_server(receiver).await;
    });
    sender
}
