use p2m::{p2m_server::P2m, Ack, Flow, Grant, IoInfo, IoResult, LocalCt, RemoteCt};
use std::collections::HashMap;
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::{mpsc, oneshot, RwLock};
use tonic::{Request, Response, Status};

use crate::containers::{ContainerAction, ContainerResult};
use crate::identifiers::Identifier;

pub mod p2m {
    tonic::include_proto!("trace2e_api");
    pub const FILE_DESCRIPTOR_SET: &[u8] = include_bytes!("../target/trace2e_api_descriptor.bin");
}

#[derive(Debug)]
pub struct P2mService {
    containers_manager: mpsc::Sender<ContainerAction>,
    identifiers_map: Arc<RwLock<HashMap<(u32, i32), Identifier>>>,
}

impl P2mService {
    pub fn new(containers_manager: mpsc::Sender<ContainerAction>) -> Self {
        P2mService {
            containers_manager: containers_manager,
            identifiers_map: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[tonic::async_trait]
impl P2m for P2mService {
    async fn local_enroll(&self, request: Request<LocalCt>) -> Result<Response<Ack>, Status> {
        let r = request.into_inner();

        #[cfg(feature = "verbose")]
        println!(
            "PID: {} | FD: {} | Local Enroll | {}",
            r.process_id.clone(),
            r.file_descriptor.clone(),
            r.path.clone()
        );

        let identifier = Identifier::File(r.path.clone());

        let (tx, rx) = oneshot::channel();
        let _ = self
            .containers_manager
            .send(ContainerAction::Register(
                Identifier::Process(r.process_id.clone()),
                tx,
            ))
            .await;
        rx.await.unwrap();

        let (tx, rx) = oneshot::channel();
        let _ = self
            .containers_manager
            .send(ContainerAction::Register(identifier.clone(), tx))
            .await;
        rx.await.unwrap();

        /*
        At the process level, there is a bijection between the file_descriptor and the designated underlying container.

        2 cases can be outlined:
        - No existing entry for tuple (process_id, file_descriptor)
          It means a process declared a new fd mapped to a container.
            => The relation must be added to the hashmap.
        - There is an entry
          It means that a process was previously declared as fd mapped to a container.
          Since the procedure was called with the existing (process_id, file_descriptor),
          it also means that the previous fd was destroyed and a new one declared,
          so that the new one could be mapped to a different container.
            => The relation must be inserted into the hashmap.

        So in any case we have to insert the relation (process_id, file_descriptor) - resource_identifier.
         */
        let mut identifiers_map = self.identifiers_map.write().await;
        identifiers_map.insert(
            (r.process_id.clone(), r.file_descriptor.clone()),
            identifier.clone(),
        );

        Ok(Response::new(Ack {}))
    }

    async fn remote_enroll(&self, request: Request<RemoteCt>) -> Result<Response<Ack>, Status> {
        let r = request.into_inner();

        #[cfg(feature = "verbose")]
        println!(
            "PID: {} | FD: {} | Remote Enroll |  [{}-{}]",
            r.process_id.clone(),
            r.file_descriptor.clone(),
            r.local_socket.clone(),
            r.peer_socket.clone()
        );

        // Check consistency of the provided sockets
        let local_socket = match r.local_socket.clone().parse::<SocketAddr>() {
            Ok(socket) => socket,
            Err(_) => {
                #[cfg(feature = "verbose")]
                eprintln!(
                    "PID: {} | FD {} | Local socket string can not be parsed.",
                    r.process_id.clone(),
                    r.file_descriptor.clone()
                );

                return Err(Status::invalid_argument(format!(
                    "Local socket string can not be parsed."
                )));
            }
        };
        let peer_socket = match r.peer_socket.clone().parse::<SocketAddr>() {
            Ok(socket) => socket,
            Err(_) => {
                #[cfg(feature = "verbose")]
                eprintln!(
                    "PID: {} | FD {} | Peer socket string can not be parsed.",
                    r.process_id.clone(),
                    r.file_descriptor.clone()
                );

                return Err(Status::invalid_argument(format!(
                    "Peer socket string can not be parsed."
                )));
            }
        };

        let identifier = Identifier::Stream(local_socket, peer_socket);

        let (tx, rx) = oneshot::channel();
        let _ = self
            .containers_manager
            .send(ContainerAction::Register(
                Identifier::Process(r.process_id.clone()),
                tx,
            ))
            .await;
        rx.await.unwrap();

        let (tx, rx) = oneshot::channel();
        let _ = self
            .containers_manager
            .send(ContainerAction::Register(identifier.clone(), tx))
            .await;
        rx.await.unwrap();

        let mut identifiers_map = self.identifiers_map.write().await;
        identifiers_map.insert(
            (r.process_id.clone(), r.file_descriptor.clone()),
            identifier.clone(),
        );

        Ok(Response::new(Ack {}))
    }

    async fn io_request(&self, request: Request<IoInfo>) -> Result<Response<Grant>, Status> {
        let r = request.into_inner();

        #[cfg(feature = "verbose")]
        println!(
            "PID: {} | FD: {} | Request for {}",
            r.process_id,
            r.file_descriptor,
            {
                if r.flow == 0 {
                    "Input/Read  "
                } else if r.flow == 1 {
                    "Output/Write"
                } else {
                    "None"
                }
            },
        );

        if let Some(resource_identifier) = self
            .identifiers_map
            .read()
            .await
            .get(&(r.process_id, r.file_descriptor))
            .cloned()
        {
            let (container_to_read, container_to_write) = match r.flow {
                flow if flow == Flow::Input.into() => (
                    resource_identifier.clone(),
                    Identifier::Process(r.process_id),
                ),
                flow if flow == Flow::Output.into() => (
                    Identifier::Process(r.process_id),
                    resource_identifier.clone(),
                ),
                _ => return Err(Status::invalid_argument(format!("Unsupported Flow type"))),
            };
            let (tx, rx) = oneshot::channel();
            let _ = self
                .containers_manager
                .send(ContainerAction::ReserveRead(container_to_read.clone(), tx))
                .await;
            match rx.await.unwrap() {
                ContainerResult::Wait(callback) => {
                    #[cfg(feature = "verbose")]
                    println!("⏸️  read wait {}", container_to_read.clone());
                    callback.await.unwrap();
                    #[cfg(feature = "verbose")]
                    println!("⏯️  read got after wait {}", container_to_read.clone())
                }
                ContainerResult::Done => {
                    #[cfg(feature = "verbose")]
                    println!("⏩ read got {}", container_to_read.clone());
                }
                _ => unimplemented!(),
            }
            let (tx, rx) = oneshot::channel();
            let _ = self
                .containers_manager
                .send(ContainerAction::ReserveWrite(
                    container_to_write.clone(),
                    tx,
                ))
                .await;
            match rx.await.unwrap() {
                ContainerResult::Wait(callback) => {
                    #[cfg(feature = "verbose")]
                    println!("⏸️  write wait {}", container_to_write.clone());
                    callback.await.unwrap();
                    #[cfg(feature = "verbose")]
                    println!("⏯️  write got after wait {}", container_to_write.clone());
                }
                ContainerResult::Done => {
                    #[cfg(feature = "verbose")]
                    println!("⏩ write got {}", container_to_write.clone());
                }
                _ => unimplemented!(),
            }
            #[cfg(feature = "verbose")]
            println!(
                "PID: {} | FD: {} | {} Authorized | {}",
                r.process_id,
                r.file_descriptor,
                {
                    if r.flow == 0 {
                        "Input/Read"
                    } else if r.flow == 1 {
                        "Output/Write"
                    } else {
                        "None"
                    }
                },
                resource_identifier.clone(),
            );
        } else {
            #[cfg(feature = "verbose")]
            eprintln!("CT {} is not tracked", r.file_descriptor);

            return Err(Status::not_found(format!(
                "CT {} is not tracked",
                r.file_descriptor
            )));
        }
        Ok(Response::new(Grant { id: 0 })) // TODO : implement grant_id
    }

    async fn io_report(&self, request: Request<IoResult>) -> Result<Response<Ack>, Status> {
        let r = request.into_inner();

        if let Some(resource_identifier) = self
            .identifiers_map
            .read()
            .await
            .get(&(r.process_id, r.file_descriptor))
            .cloned()
        {
            let (tx, rx) = oneshot::channel();
            self.containers_manager
                .send(ContainerAction::Release(
                    Identifier::Process(r.process_id.clone()),
                    tx,
                ))
                .await
                .unwrap();
            rx.await.unwrap();
            let (tx, rx) = oneshot::channel();
            self.containers_manager
                .send(ContainerAction::Release(resource_identifier.clone(), tx))
                .await
                .unwrap();
            rx.await.unwrap();

            #[cfg(feature = "verbose")]
            println!(
                "PID: {} | FD: {} | IO Done | {}",
                r.process_id, r.file_descriptor, resource_identifier.clone()
            );
        } else {
            #[cfg(feature = "verbose")]
            eprintln!("CT {} is not tracked", r.file_descriptor);

            return Err(Status::not_found(format!(
                "CT {} is not tracked",
                r.file_descriptor
            )));
        }

        Ok(Response::new(Ack {}))
    }
}
