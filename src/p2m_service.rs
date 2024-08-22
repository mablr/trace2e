use crate::containers::ContainerAction;
use crate::identifiers::Identifier;
use crate::provenance::{Flow as ProvFlow, ProvenanceLayer};
use p2m::{p2m_server::P2m, Ack, Flow, Grant, IoInfo, IoResult, LocalCt, RemoteCt};
use procfs::process::Process;
use std::collections::HashMap;
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use tonic::{Request, Response, Status};

pub mod p2m {
    tonic::include_proto!("trace2e_api");
    pub const FILE_DESCRIPTOR_SET: &[u8] = include_bytes!("../target/trace2e_api_descriptor.bin");
}

#[derive(Debug)]
pub struct P2mService {
    containers_manager: mpsc::Sender<ContainerAction>,
    identifiers_map: Arc<RwLock<HashMap<(u32, i32), Identifier>>>,
    provenance: ProvenanceLayer,
    flows: Arc<Mutex<HashMap<u64, ProvFlow>>>,
}

impl P2mService {
    pub fn new(containers_manager: mpsc::Sender<ContainerAction>) -> Self {
        P2mService {
            containers_manager: containers_manager.clone(),
            identifiers_map: Arc::new(RwLock::new(HashMap::new())),
            provenance: ProvenanceLayer::new(containers_manager),
            flows: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[tonic::async_trait]
impl P2m for P2mService {
    async fn local_enroll(&self, request: Request<LocalCt>) -> Result<Response<Ack>, Status> {
        let r = request.into_inner();

        let process_starttime = match Process::new(r.process_id.try_into().unwrap())
            .and_then(|p| p.stat())
            .map(|stat| stat.starttime)
        {
            Ok(starttime) => starttime,
            Err(e) => return Err(Status::from_error(Box::new(e))),
        };
        let process_identifier =
            Identifier::Process(r.process_id.try_into().unwrap(), process_starttime);

        let resource_identifier = Identifier::File(r.path.clone());

        #[cfg(feature = "verbose")]
        println!(
            "PID: {} | FD: {} | Local Enroll | {}",
            r.process_id.clone(),
            r.file_descriptor.clone(),
            r.path.clone()
        );

        let (tx, rx) = oneshot::channel();
        let _ = self
            .containers_manager
            .send(ContainerAction::Register(process_identifier, tx))
            .await;
        rx.await.unwrap();

        let (tx, rx) = oneshot::channel();
        let _ = self
            .containers_manager
            .send(ContainerAction::Register(resource_identifier.clone(), tx))
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
        identifiers_map.insert((r.process_id, r.file_descriptor), resource_identifier);

        Ok(Response::new(Ack {}))
    }

    async fn remote_enroll(&self, request: Request<RemoteCt>) -> Result<Response<Ack>, Status> {
        let r = request.into_inner();

        let process_starttime = match Process::new(r.process_id.try_into().unwrap())
            .and_then(|p| p.stat())
            .map(|stat| stat.starttime)
        {
            Ok(starttime) => starttime,
            Err(e) => return Err(Status::from_error(Box::new(e))),
        };
        let process_identifier = Identifier::Process(r.process_id, process_starttime);

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
            .send(ContainerAction::Register(process_identifier, tx))
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

        let process_starttime = match Process::new(r.process_id.try_into().unwrap())
            .and_then(|p| p.stat())
            .map(|stat| stat.starttime)
        {
            Ok(starttime) => starttime,
            Err(e) => return Err(Status::from_error(Box::new(e))),
        };
        let process_identifier = Identifier::Process(r.process_id, process_starttime);

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
            let flow = match r.flow {
                flow_type if flow_type == Flow::Input.into() => self
                    .provenance
                    .declare_flow(resource_identifier.clone(), process_identifier)
                    .await
                    .unwrap(),
                flow_type if flow_type == Flow::Output.into() => self
                    .provenance
                    .declare_flow(process_identifier, resource_identifier.clone())
                    .await
                    .unwrap(),
                _ => return Err(Status::invalid_argument(format!("Unsupported Flow type"))),
            };
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
            let mut flows = self.flows.lock().await;
            flows.insert(flow.id, flow.clone());
            Ok(Response::new(Grant { id: flow.id }))
        } else {
            #[cfg(feature = "verbose")]
            eprintln!("CT {} is not tracked", r.file_descriptor);

            return Err(Status::not_found(format!(
                "CT {} is not tracked",
                r.file_descriptor
            )));
        }
    }

    async fn io_report(&self, request: Request<IoResult>) -> Result<Response<Ack>, Status> {
        let r = request.into_inner();

        if let Some(_resource_identifier) = self
            .identifiers_map
            .read()
            .await
            .get(&(r.process_id, r.file_descriptor))
            .cloned()
        {
            if let Some(flow) = self.flows.lock().await.get(&(r.grant_id)).cloned() {
                match self.provenance.record_flow(flow).await {
                    Ok(_) => {
                        #[cfg(feature = "verbose")]
                        println!(
                            "PID: {} | FD: {} | IO Done | {}",
                            r.process_id,
                            r.file_descriptor,
                            _resource_identifier.clone()
                        );

                        Ok(Response::new(Ack {}))
                    }
                    Err(_) => {
                        #[cfg(feature = "verbose")]
                        eprintln!("Provenance recording failure, Flow {}", r.grant_id);

                        Err(Status::internal("Provenance recording failure"))
                    }
                }
            } else {
                #[cfg(feature = "verbose")]
                eprintln!("Flow {} does not exist", r.grant_id);

                return Err(Status::not_found(format!(
                    "Flow {} does not exist",
                    r.grant_id
                )));
            }
        } else {
            #[cfg(feature = "verbose")]
            eprintln!("CT {} is not tracked", r.file_descriptor);

            return Err(Status::not_found(format!(
                "CT {} is not tracked",
                r.file_descriptor
            )));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::containers::containers_manager;
    use std::process::Command;

    #[tokio::test]
    async fn p2m_scenario_1p_1f_write() -> Result<(), Box<dyn std::error::Error>> {
        let (sender, receiver) = mpsc::channel(32);
        tokio::spawn(containers_manager(receiver));
        let client = P2mService::new(sender);
        let mut process = Command::new("tail").arg("-f").arg("/dev/null").spawn()?;

        // CT declaration
        let file_creation = tonic::Request::new(LocalCt {
            process_id: process.id(),
            file_descriptor: 3,
            path: "bar.txt".to_string(),
        });
        let result_file_creation = client.local_enroll(file_creation).await?.into_inner();
        assert_eq!(result_file_creation, Ack {});

        // Write event
        let write_request = tonic::Request::new(IoInfo {
            process_id: process.id(),
            file_descriptor: 3,
            flow: Flow::Output.into(),
        });
        let result_write_request = client.io_request(write_request).await?;
        let grant_id = result_write_request.into_inner().id;

        // Write done
        let write_done = tonic::Request::new(IoResult {
            process_id: process.id(),
            file_descriptor: 3,
            grant_id,
            result: true,
        });
        let result_write_done = client.io_report(write_done).await?.into_inner();
        assert_eq!(result_write_done, Ack {});

        process.kill()?;
        Ok(())
    }

    #[tokio::test]
    async fn p2m_scenario_1p_1f_read() -> Result<(), Box<dyn std::error::Error>> {
        let (sender, receiver) = mpsc::channel(32);
        tokio::spawn(containers_manager(receiver));
        let client = P2mService::new(sender);
        let mut process = Command::new("tail").arg("-f").arg("/dev/null").spawn()?;

        // CT declaration
        let file_creation = tonic::Request::new(LocalCt {
            process_id: process.id(),
            file_descriptor: 3,
            path: "bar.txt".to_string(),
        });
        let result_file_creation = client.local_enroll(file_creation).await?.into_inner();
        assert_eq!(result_file_creation, Ack {});

        // Read event
        let read_request = tonic::Request::new(IoInfo {
            process_id: process.id(),
            file_descriptor: 3,
            flow: Flow::Input.into(),
        });
        let result_read_request = client.io_request(read_request).await?;
        let grant_id = result_read_request.into_inner().id;

        // Read done
        let read_done = tonic::Request::new(IoResult {
            process_id: process.id(),
            file_descriptor: 3,
            grant_id,
            result: true,
        });
        let result_read_done = client.io_report(read_done).await?.into_inner();
        assert_eq!(result_read_done, Ack {});

        process.kill()?;
        Ok(())
    }

    #[tokio::test]
    async fn p2m_scenario_1p_1s_write() -> Result<(), Box<dyn std::error::Error>> {
        let (sender, receiver) = mpsc::channel(32);
        tokio::spawn(containers_manager(receiver));
        let client = P2mService::new(sender);
        let mut process = Command::new("tail").arg("-f").arg("/dev/null").spawn()?;

        // CT declaration
        let stream_creation = tonic::Request::new(RemoteCt {
            process_id: process.id(),
            file_descriptor: 3,
            local_socket: "10.0.0.1:54321".to_string(),
            peer_socket: "10.0.0.2:8081".to_string(),
        });
        let result_stream_creation = client.remote_enroll(stream_creation).await?.into_inner();
        assert_eq!(result_stream_creation, Ack {});

        // Write event
        let write_request = tonic::Request::new(IoInfo {
            process_id: process.id(),
            file_descriptor: 3,
            flow: Flow::Output.into(),
        });
        let result_write_request = client.io_request(write_request).await?;
        let grant_id = result_write_request.into_inner().id;

        // Write done
        let write_done = tonic::Request::new(IoResult {
            process_id: process.id(),
            file_descriptor: 3,
            grant_id,
            result: true,
        });
        let result_write_done = client.io_report(write_done).await?.into_inner();
        assert_eq!(result_write_done, Ack {});

        process.kill()?;
        Ok(())
    }

    #[tokio::test]
    async fn p2m_scenario_1p_1s_read() -> Result<(), Box<dyn std::error::Error>> {
        let (sender, receiver) = mpsc::channel(32);
        tokio::spawn(containers_manager(receiver));
        let client = P2mService::new(sender);
        let mut process = Command::new("tail").arg("-f").arg("/dev/null").spawn()?;

        // CT declaration
        let stream_creation = tonic::Request::new(RemoteCt {
            process_id: process.id(),
            file_descriptor: 3,
            local_socket: "10.0.0.1:54321".to_string(),
            peer_socket: "10.0.0.2:8081".to_string(),
        });
        let result_stream_creation = client.remote_enroll(stream_creation).await?.into_inner();
        assert_eq!(result_stream_creation, Ack {});

        // Read event
        let read_request = tonic::Request::new(IoInfo {
            process_id: process.id(),
            file_descriptor: 3,
            flow: Flow::Input.into(),
        });
        let result_read_request = client.io_request(read_request).await?;
        let grant_id = result_read_request.into_inner().id;

        // Read done
        let read_done = tonic::Request::new(IoResult {
            process_id: process.id(),
            file_descriptor: 3,
            grant_id,
            result: true,
        });
        let result_read_done = client.io_report(read_done).await?.into_inner();
        assert_eq!(result_read_done, Ack {});

        process.kill()?;
        Ok(())
    }
}
