///! Provenance layer module.
///!
///!
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{
    sync::{mpsc, oneshot, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard},
    time::timeout,
};
use tonic::Request;

use crate::{
    identifier::Identifier,
    labels::Labels,
    m2m_service::m2m::{m2m_client::M2mClient, Id, Stream, StreamProv},
};

use super::ProvenanceError;

/// This function checks if the flow involves one process and one non-process
/// containers, if so the process ID is returned.
fn validate_flow(id1: Identifier, id2: Identifier) -> Option<u32> {
    if id2.is_process().is_none() {
        id1.is_process()
    } else {
        None
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

/// Provenance layer response message type.
#[derive(Debug)]
pub enum ProvenanceResult {
    Registered,
    Declared(u64),
    Recorded,
    Error(ProvenanceError),
    WaitingSync(oneshot::Sender<(Vec<Identifier>, oneshot::Sender<ProvenanceResult>)>),
}

impl ProvenanceResult {
    /// Returns `true` if the provenance result is [`WaitingSync`].
    ///
    /// [`WaitingSync`]: ProvenanceResult::WaitingSync
    #[must_use]
    pub fn is_waiting_sync(&self) -> bool {
        matches!(self, Self::WaitingSync(..))
    }
}

/// Provenance layer request message type.
pub enum ProvenanceAction {
    RegisterContainer(Identifier, oneshot::Sender<ProvenanceResult>),
    DeclareFlow(
        Identifier,
        Identifier,
        bool,
        oneshot::Sender<ProvenanceResult>,
    ),
    RecordFlow(u64, oneshot::Sender<ProvenanceResult>),
    SyncStream(
        Identifier,
        oneshot::Sender<ProvenanceResult>,
    ),
}

/// This asynchronous function is responsible of provenance tracking, it
/// provides a global consistent provenance state in a fully asynchronous
/// paradigm.
///
/// To do so, it listens to a [`tokio`] mpsc channel for incoming
/// [`ProvenanceAction`] messages and processes them accordingly.
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
/// [`ProvenanceAction`] messages.
///
/// # Behavior
///
/// The function handles 3 different actions matched with the variants of the
/// [`ProvenanceAction`] enum:
///
/// 1. [`ProvenanceAction::RegisterContainer`]:
///    - Registers a new container identified by a unique [`Identifier`] object
///     if it is a File or a Process and not already registered (overwritting
///     forbidden), else if it is a Stream (overwritting allowed).
///    - Sends a `ProvenanceResult::Registered` back through the responder once
///     the registration is successful.
///
/// 2. [`ProvenanceAction::DeclareFlow`]:
///    - Validates the flow : checks if the involved containers are registered,
///     and if the containers types are valid.
///    - If the flow is valid, it spawns an asynchronous task to manage the flow
///     reservation (see [Flow reservation
///     mechanism](#flow-reservation-mechanism) for more details).
///
/// 3. [`ProvenanceAction::RecordFlow`]:
///    - If the grant ID is found, it sends a signal to the task that holds the
///     corresponding flow reservation to record the flow in the provenance,
///     then to drop then the reservation of the containers and it returns a
///     [`ProvenanceResult::Recorded`] message.
///    - Otherwise, it returns a [`ProvenanceResult`] containing to following
///     error [`ProvenanceError::RecordingFailure`].
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
/// container is written. The provenance layer propagates the provenance
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
/// A message `ProvenanceResult::Declared` with a unique grant ID confirming the
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
/// runtime. It continuously listens for [`ProvenanceAction`] messages and
/// processes them based on their variant.
///
/// ```rust
/// use tokio::sync::mpsc;
///
/// #[tokio::main]
/// async fn main() {
///     let (sender, receiver) = mpsc::channel(32);
///
///     // Spawn the provenance layer to handle messages
///     tokio::spawn(async move {
///         provenance_layer(receiver).await;
///     });
///
///     // Example sending messages to the provenance layer
///     let (tx, rx) = oneshot::channel();
///     sender.send(ProvenanceAction::RegisterContainer(Identifier::File("/path/to/file"), rx)).await.unwrap();
///     rx.await.unwrap();
/// }
/// ```
///
/// # Notes
/// This function provides verbose logging (enabled with "verbose" feature flag)
/// for a live view of the provenance layer operations.
///
/// [`tokio`]: https://docs.rs/tokio
pub async fn provenance_layer(mut receiver: mpsc::Receiver<ProvenanceAction>) {
    let mut containers = HashMap::new();
    let grant_counter: Arc<Mutex<u64>> = Arc::new(Mutex::new(0));
    let flows_release_handles = Arc::new(Mutex::new(HashMap::new()));

    while let Some(message) = receiver.recv().await {
        match message {
            ProvenanceAction::RegisterContainer(identifier, responder) => {
                if !containers.contains_key(&identifier) || identifier.is_stream().is_some() {
                    containers.insert(
                        identifier.clone(),
                        Arc::new(RwLock::new(Labels::new(identifier.clone()))),
                    );
                }
                responder.send(ProvenanceResult::Registered).unwrap();
            }
            ProvenanceAction::DeclareFlow(id1, id2, output, responder) => {
                if let Some(pid) = validate_flow(id1.clone(), id2.clone()) {
                    if let (Some(id1_container), Some(id2_container)) =
                        (containers.get(&id1).cloned(), containers.get(&id2).cloned())
                    {
                        let grant_counter = Arc::clone(&grant_counter);
                        let flows_release_handles = Arc::clone(&flows_release_handles);
                        tokio::spawn(async move {
                            let grant_id = {
                                let mut grant_counter = grant_counter.lock().await;
                                *grant_counter += 1;
                                *grant_counter
                            };
                            if let Some((local_socket, peer_socket)) = id2.is_stream().filter(|_| output) {
                            // Output Flow to stream (Decentralized procedure)
                                if let Ok(mut client) = M2mClient::connect(format!("http://{}:8080", peer_socket.ip())).await {
                                    let source_labels = id1_container.read().await;

                                    if let Ok(_) = client.reserve(Request::new(Stream {
                                        local_socket: local_socket.to_string(),
                                        peer_socket: peer_socket.to_string(),
                                    })).await {
                                        let (release_callback, release) = oneshot::channel();
                                        flows_release_handles
                                            .lock()
                                            .await
                                            .insert(grant_id, release_callback);

                                        responder
                                            .send(ProvenanceResult::Declared(grant_id))
                                            .unwrap();

                                        if timeout(Duration::from_millis(50), release).await.is_err() {
                                            #[cfg(feature = "verbose")]
                                            println!("⚠️  Reservation timeout Flow {}", grant_id);
            
                                            // Remove flow release handle
                                            flows_release_handles.lock().await.remove(&grant_id);

                                            // Try to kill the process that holds the reservation for too long,
                                            // to release the reservation safely
                                            let _ = std::process::Command::new("kill")
                                                .arg("-9")
                                                .arg(pid.to_string())
                                                .status();
                                            // Todo: handle the result ?

                                            // Release remote stream
                                            let _ = client.sync_provenance(Request::new(StreamProv {
                                                local_socket: local_socket.to_string(),
                                                peer_socket: peer_socket.to_string(),
                                                provenance: Vec::new() // empty provenance to skip update and release remote stream
                                            })).await;
                                        } else {
                                            #[cfg(feature = "verbose")]
                                            println!(
                                                "🔼  Flow {} Sync to {} ({:?} -> {:?})",
                                                grant_id,
                                                peer_socket.ip(),
                                                id1.clone(),
                                                id2.clone()
                                            );
                                            let _ = client.sync_provenance(Request::new(StreamProv {
                                                local_socket: local_socket.to_string(),
                                                peer_socket: peer_socket.to_string(),
                                                provenance: source_labels
                                                    .get_prov()
                                                    .into_iter()
                                                    .map(Id::from)
                                                    .collect(),
                                            })).await; // Todo Sync failure management
                                        }
                                    }

                                }
                            } else {
                            // File IO Flow, or Input flow from Stream (Local procedure)
                                let (source_labels, mut destination_labels) =
                                    reserve_local_flow(output, &id1_container, &id2_container).await;

                                let (release_callback, release) = oneshot::channel();
                                flows_release_handles
                                    .lock()
                                    .await
                                    .insert(grant_id, release_callback);



                                responder
                                    .send(ProvenanceResult::Declared(grant_id))
                                    .unwrap();

                                #[cfg(feature = "verbose")]
                                println!(
                                    "✅ Flow {} granted ({:?} {} {:?})",
                                    grant_id,
                                    id1.clone(),
                                    if output { "->" } else { "<-" },
                                    id2.clone()
                                );

                                if timeout(Duration::from_millis(50), release).await.is_err() {
                                    #[cfg(feature = "verbose")]
                                    println!("⚠️  Reservation timeout Flow {}", grant_id);
    
                                    // Remove flow release handle
                                    flows_release_handles.lock().await.remove(&grant_id);
    
                                    // Try to kill the process that holds the reservation for too long,
                                    // to release the reservation safely
                                    let _ = std::process::Command::new("kill")
                                        .arg("-9")
                                        .arg(pid.to_string())
                                        .status();
                                    // Todo: handle the result ?
                                } else {
                                    // Record flow locally if it is not an output to a stream
                                    #[cfg(feature = "verbose")]
                                    println!(
                                        "⏺️  Flow {} recording ({:?} {} {:?})",
                                        grant_id,
                                        id1.clone(),
                                        if output { "->" } else { "<-" },
                                        id2.clone()
                                    );
                                    destination_labels.update_prov(&source_labels);
                                    #[cfg(feature = "verbose")]
                                    println!(
                                        "🆕 Provenance: {{{:?}: {:?},  {:?}: {:?}}}",
                                        if output { id1.clone() } else { id2.clone() },
                                        source_labels.get_prov(),
                                        if output { id2.clone() } else { id1.clone() },
                                        destination_labels.get_prov(),
                                    );
                                }
                            }

                            #[cfg(feature = "verbose")]
                            println!("🗑️  Flow {} destruction", grant_id);
                        });
                    } else {
                        responder
                            .send(ProvenanceResult::Error(
                                ProvenanceError::MissingRegistration(id1, id2),
                            ))
                            .unwrap();
                    }
                } else {
                    responder
                        .send(ProvenanceResult::Error(ProvenanceError::InvalidFlow(
                            id1, id2,
                        )))
                        .unwrap();
                }
            }
            ProvenanceAction::RecordFlow(grant_id, responder) => {
                if let Some(release_handle) = flows_release_handles.lock().await.remove(&grant_id) {
                    release_handle.send(()).unwrap();
                    // No wait for release feedback, the consistency is guaranted by RwLock<> structure properties.
                    responder.send(ProvenanceResult::Recorded).unwrap();
                } else {
                    responder
                        .send(ProvenanceResult::Error(ProvenanceError::RecordingFailure(
                            grant_id,
                        )))
                        .unwrap();
                }
            }
            ProvenanceAction::SyncStream(id, responder) => {
                if let Some(id_container) = id.is_stream().and(containers.get(&id)).cloned() {
                    tokio::spawn(async move {
                        let mut stream_labels = id_container.write().await;
                        let (provenance_sender, provenance_receiver) = oneshot::channel();
                        responder.send(ProvenanceResult::WaitingSync(provenance_sender)).unwrap();

                        match provenance_receiver.await {
                            Ok((provenance, callback)) => {
                                stream_labels.set_prov(provenance);
                                let _ = callback.send(ProvenanceResult::Recorded);
                            }
                            Err(_) => todo!(),
                        }
                        #[cfg(feature = "verbose")]
                        println!(
                            "🔽 Remote provenance sync on {:?}",
                            id.clone(),
                        );
                    });
                } else {
                    responder
                        .send(ProvenanceResult::Error(ProvenanceError::SyncFailure(id)))
                        .unwrap();
                }
            }
        }
    }
}
