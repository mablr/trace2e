use std::{collections::HashMap, sync::Arc, time::Duration};

use tokio::{
    sync::{mpsc, oneshot, Mutex, RwLock},
    time::timeout,
};

use crate::identifiers::Identifier;

use super::{Labels, ProvenanceError};

fn flow_is_valid(id1: Identifier, id2: Identifier) -> bool {
    match id1 {
        Identifier::Process(..) => match id2 {
            Identifier::Process(..) => false,
            _ => true,
        },
        _ => false,
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ProvenanceResult {
    Registered,
    Declared(u64),
    Recorded,
    Error(ProvenanceError),
}

pub enum ProvenanceAction {
    RegisterContainer(Identifier, oneshot::Sender<ProvenanceResult>),
    DeclareFlow(
        Identifier,
        Identifier,
        bool,
        oneshot::Sender<ProvenanceResult>,
    ),
    RecordFlow(u64, oneshot::Sender<ProvenanceResult>),
}

pub async fn provenance_layer(mut receiver: mpsc::Receiver<ProvenanceAction>) {
    let mut containers = HashMap::new();
    let grant_counter: Arc<Mutex<u64>> = Arc::new(Mutex::new(0));
    let flows_release_handles = Arc::new(Mutex::new(HashMap::new()));

    while let Some(message) = receiver.recv().await {
        match message {
            ProvenanceAction::RegisterContainer(identifier, responder) => {
                if !containers.contains_key(&identifier) {
                    containers.insert(identifier.clone(), Arc::new(RwLock::new(Labels::new(identifier.clone()))));
                }
                responder.send(ProvenanceResult::Registered).unwrap();
            }
            ProvenanceAction::DeclareFlow(id1, id2, output, responder) => {
                if flow_is_valid(id1.clone(), id2.clone()) {
                    if let (Some(id1_container), Some(id2_container)) =
                        (containers.get(&id1).cloned(), containers.get(&id2).cloned())
                    {
                        let grant_counter = Arc::clone(&grant_counter);
                        let flows_release_handles = Arc::clone(&flows_release_handles);
                        tokio::spawn(async move {
                            let (source_labels, mut destination_labels) = {
                                if output {
                                    #[cfg(feature = "verbose")]
                                    println!("⏸️  read wait for {}", id1.clone());
                                    let rguard = id1_container.read().await;
                                    #[cfg(feature = "verbose")]
                                    println!("⏯️  read got for {}", id1.clone());
                                    #[cfg(feature = "verbose")]
                                    println!("⏸️  write wait {}", id2.clone());
                                    let wguard = id2_container.write().await;
                                    #[cfg(feature = "verbose")]
                                    println!("⏯️  write got for {}", id2.clone());
                                    (rguard, wguard)
                                } else {
                                    #[cfg(feature = "verbose")]
                                    println!("⏸️  write wait for {}", id1.clone());
                                    let wguard = id1_container.write().await;
                                    #[cfg(feature = "verbose")]
                                    println!("⏯️  write got for {}", id1.clone());
                                    #[cfg(feature = "verbose")]
                                    println!("⏸️  read wait for {}", id2.clone());
                                    let rguard = id2_container.read().await;
                                    #[cfg(feature = "verbose")]
                                    println!("⏯️  read got for {}", id2.clone());
                                    (rguard, wguard)
                                }
                            };

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
                                .send(ProvenanceResult::Declared(grant_id))
                                .unwrap();

                            #[cfg(feature = "verbose")]
                            println!(
                                "✅  Flow {} granted ({} {} {})",
                                grant_id,
                                id1.clone(),
                                if output {"->"} else {"<-"},
                                id2.clone()
                            );

                            if timeout(Duration::from_millis(50), release).await.is_err() {
                                #[cfg(feature = "verbose")]
                                println!("⚠️  Reservation timeout Flow {}", grant_id);

                                #[cfg(not(test))]
                                todo!(); // todo kill the blocking process

                                #[cfg(test)]
                                panic!("⚠️  Reservation timeout");
                            } else {
                                #[cfg(feature = "verbose")]
                                println!(
                                    "⏺️  Flow {} recording ({} {} {})",
                                    grant_id,
                                    id1.clone(),
                                    if output {"->"} else {"<-"},
                                    id2.clone()
                                );

                                destination_labels.update_prov(&source_labels);

                                #[cfg(feature = "verbose")]
                                if output {
                                    println!(
                                        "🆕 Provenance: {{{}: {:?},  {}: {:?}}}",
                                        id1.clone(),
                                        source_labels.get_prov(),
                                        id2.clone(),
                                        destination_labels.get_prov(),
                                    );
                                } else {
                                    println!(
                                        "🆕 Provenance: {{{}: {:?},  {}: {:?}}}",
                                        id2.clone(),
                                        source_labels.get_prov(),
                                        id1.clone(),
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
                                ProvenanceError::DeclarationFailure(id1, id2), // todo specific error not declared container ?
                            ))
                            .unwrap();
                    }
                } else {
                    responder
                        .send(ProvenanceResult::Error(
                            ProvenanceError::DeclarationFailure(id1, id2), // todo specific error invalid flow ?
                        ))
                        .unwrap();
                }
            }
            ProvenanceAction::RecordFlow(grant_id, responder) => {
                if let Some(handle) = flows_release_handles.lock().await.remove(&grant_id) {
                    handle.send(()).unwrap();
                    responder.send(ProvenanceResult::Recorded).unwrap();
                } else {
                    responder
                        .send(ProvenanceResult::Error(ProvenanceError::RecordingFailure(
                            grant_id,
                        )))
                        .unwrap();
                }
            }
        }
    }
}
