//! Processes to Middleware gRPC Service.

use crate::{
    identifier::Identifier,
    provenance::{ProvenanceAction, ProvenanceResult},
};
use p2m::{p2m_server::P2m, Ack, Flow, Grant, IoInfo, IoResult, LocalCt, RemoteCt};
use procfs::process::Process;
use std::collections::HashMap;
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::time::Instant;
use tonic::{Request, Response, Status};
use tracing::{error, info, instrument};

pub mod p2m {
    tonic::include_proto!("p2m_api");
    pub const P2M_DESCRIPTOR_SET: &[u8] = include_bytes!("../target/p2m_descriptor.bin");
}

#[derive(Debug)]
pub struct P2mService {
    identifiers_map: Arc<RwLock<HashMap<(u32, i32), Identifier>>>,
    provenance: mpsc::Sender<ProvenanceAction>,
}

impl P2mService {
    pub fn new(provenance_layer: mpsc::Sender<ProvenanceAction>) -> Self {
        P2mService {
            identifiers_map: Arc::new(RwLock::new(HashMap::new())),
            provenance: provenance_layer,
        }
    }
}

#[tonic::async_trait]
impl P2m for P2mService {
    #[instrument(skip(self, request))]
    async fn local_enroll(&self, request: Request<LocalCt>) -> Result<Response<Ack>, Status> {
        let start_time = Instant::now();

        let r = request.into_inner();

        let process_starttime = match Process::new(r.process_id.try_into().unwrap())
            .and_then(|p| p.stat())
            .map(|stat| stat.starttime)
        {
            Ok(starttime) => starttime,
            Err(_) => {
                return Err(Status::not_found(format!(
                    "Process {} not found.",
                    r.process_id
                )))
            }
        };
        let process_identifier =
            Identifier::new_process(r.process_id.try_into().unwrap(), process_starttime);

        let resource_identifier = Identifier::new_file(r.path.clone());

        info!(
            "PID: {} | FD: {} | {}",
            r.process_id.clone(),
            r.file_descriptor.clone(),
            r.path.clone()
        );

        let (tx, rx) = oneshot::channel();
        let _ = self
            .provenance
            .send(ProvenanceAction::RegisterContainer(process_identifier, tx))
            .await;
        rx.await.unwrap();

        let (tx, rx) = oneshot::channel();
        let _ = self
            .provenance
            .send(ProvenanceAction::RegisterContainer(
                resource_identifier.clone(),
                tx,
            ))
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

        info!("completed in {:?}", start_time.elapsed());

        Ok(Response::new(Ack {}))
    }

    #[instrument(skip(self, request))]
    async fn remote_enroll(&self, request: Request<RemoteCt>) -> Result<Response<Ack>, Status> {
        let start_time = Instant::now();

        let r = request.into_inner();

        let process_starttime = match Process::new(r.process_id.try_into().unwrap())
            .and_then(|p| p.stat())
            .map(|stat| stat.starttime)
        {
            Ok(starttime) => starttime,
            Err(_) => {
                return Err(Status::not_found(format!(
                    "Process {} not found.",
                    r.process_id
                )))
            }
        };
        let process_identifier = Identifier::new_process(r.process_id, process_starttime);

        info!(
            "PID: {} | FD: {} | [{}-{}]",
            r.process_id.clone(),
            r.file_descriptor.clone(),
            r.local_socket.clone(),
            r.peer_socket.clone()
        );

        // Check consistency of the provided sockets
        let local_socket = match r.local_socket.clone().parse::<SocketAddr>() {
            Ok(socket) => socket,
            Err(_) => {
                error!(
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
                error!(
                    "PID: {} | FD {} | Peer socket string can not be parsed.",
                    r.process_id.clone(),
                    r.file_descriptor.clone()
                );

                return Err(Status::invalid_argument(format!(
                    "Peer socket string can not be parsed."
                )));
            }
        };

        let identifier = Identifier::new_stream(local_socket, peer_socket);

        let (tx, rx) = oneshot::channel();
        let _ = self
            .provenance
            .send(ProvenanceAction::RegisterContainer(process_identifier, tx))
            .await;
        rx.await.unwrap();

        let (tx, rx) = oneshot::channel();
        let _ = self
            .provenance
            .send(ProvenanceAction::RegisterContainer(identifier.clone(), tx))
            .await;
        rx.await.unwrap();

        let mut identifiers_map = self.identifiers_map.write().await;
        identifiers_map.insert(
            (r.process_id.clone(), r.file_descriptor.clone()),
            identifier.clone(),
        );

        info!("completed in {:?}", start_time.elapsed());

        Ok(Response::new(Ack {}))
    }

    #[instrument(skip(self, request))]
    async fn io_request(&self, request: Request<IoInfo>) -> Result<Response<Grant>, Status> {
        let start_time = Instant::now();

        let r = request.into_inner();

        let process_starttime = match Process::new(r.process_id.try_into().unwrap())
            .and_then(|p| p.stat())
            .map(|stat| stat.starttime)
        {
            Ok(starttime) => starttime,
            Err(_) => {
                return Err(Status::not_found(format!(
                    "Process {} not found.",
                    r.process_id
                )))
            }
        };
        let process_identifier = Identifier::new_process(r.process_id, process_starttime);

        info!("PID: {} | FD: {} | {}", r.process_id, r.file_descriptor, {
            if r.flow == 0 {
                "Input/Read  "
            } else if r.flow == 1 {
                "Output/Write"
            } else {
                "None"
            }
        },);

        if let Some(resource_identifier) = self
            .identifiers_map
            .read()
            .await
            .get(&(r.process_id, r.file_descriptor))
            .cloned()
        {
            let (tx, rx) = oneshot::channel();
            let _ = match r.flow {
                flow_type if flow_type == Flow::Input.into() => {
                    self.provenance
                        .send(ProvenanceAction::DeclareFlow(
                            process_identifier.clone(),
                            resource_identifier.clone(),
                            false,
                            tx,
                        ))
                        .await
                }
                flow_type if flow_type == Flow::Output.into() => {
                    self.provenance
                        .send(ProvenanceAction::DeclareFlow(
                            process_identifier.clone(),
                            resource_identifier.clone(),
                            true,
                            tx,
                        ))
                        .await
                }
                _ => return Err(Status::invalid_argument(format!("Unsupported Flow type."))),
            };

            let grant_id = match rx.await.unwrap() {
                ProvenanceResult::Declared(grant_id) => grant_id,
                ProvenanceResult::Error(e) => {
                    error!("{}", e);

                    return Err(Status::from_error(Box::new(e)));
                }
                _ => unreachable!(),
            };

            info!(
                "PID: {} | FD: {} | {} granted in {:?} | {:?}",
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
                start_time.elapsed(),
                resource_identifier.clone(),
            );

            Ok(Response::new(Grant { id: grant_id }))
        } else {
            error!(
                "Process {} has not enrolled FD {}.",
                r.process_id, r.file_descriptor
            );

            Err(Status::not_found(format!(
                "Process {} has not enrolled FD {}.",
                r.process_id, r.file_descriptor
            )))
        }
    }

    #[instrument(skip(self, request))]
    async fn io_report(&self, request: Request<IoResult>) -> Result<Response<Ack>, Status> {
        let start_time = Instant::now();

        let r = request.into_inner();

        if let Some(resource_identifier) = self
            .identifiers_map
            .read()
            .await
            .get(&(r.process_id, r.file_descriptor))
            .cloned()
        {
            let (tx, rx) = oneshot::channel();
            let _ = self
                .provenance
                .send(ProvenanceAction::RecordFlow(r.grant_id, tx))
                .await;
            match rx.await.unwrap() {
                ProvenanceResult::Recorded => {
                    info!(
                        "PID: {} | FD: {} | done in {:?} | {:?}",
                        r.process_id,
                        r.file_descriptor,
                        start_time.elapsed(),
                        resource_identifier.clone()
                    );

                    Ok(Response::new(Ack {}))
                }
                ProvenanceResult::Error(e) => {
                    error!("{}", e);

                    Err(Status::from_error(Box::new(e)))
                }
                _ => unreachable!(),
            }
        } else {
            error!(
                "Process {} has not enrolled FD {}.",
                r.process_id, r.file_descriptor
            );

            Err(Status::not_found(format!(
                "Process {} has not enrolled FD {}.",
                r.process_id, r.file_descriptor
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use crate::provenance::provenance_layer;

    use super::*;

    #[tokio::test]
    async fn unit_p2m_enroll_failure() -> Result<(), Box<dyn std::error::Error>> {
        let (sender, receiver) = mpsc::channel(32);
        tokio::spawn(provenance_layer(receiver));
        let client = P2mService::new(sender);
        let mut process = Command::new("tail").arg("-f").arg("/dev/null").spawn()?;

        let file_creation1 = tonic::Request::new(LocalCt {
            process_id: 0,
            file_descriptor: 3,
            path: "bar.txt".to_string(),
        });
        let file_status1 = client.local_enroll(file_creation1).await.unwrap_err();
        assert_eq!(file_status1.message(), "Process 0 not found.");

        let stream_creation1 = tonic::Request::new(RemoteCt {
            process_id: 0,
            file_descriptor: 4,
            local_socket: "[ffc7::1]:54321".to_string(),
            peer_socket: "ffc7::2:8081".to_string(),
        });
        let stream_status1 = client.remote_enroll(stream_creation1).await.unwrap_err();
        assert_eq!(stream_status1.message(), "Process 0 not found.");

        let stream_creation2 = tonic::Request::new(RemoteCt {
            process_id: process.id(),
            file_descriptor: 5,
            local_socket: "10.INVALID1:54321".to_string(),
            peer_socket: "10.0.0.2:8081".to_string(),
        });
        let stream_status2 = client.remote_enroll(stream_creation2).await.unwrap_err();
        assert_eq!(
            stream_status2.message(),
            "Local socket string can not be parsed."
        );

        let stream_creation3 = tonic::Request::new(RemoteCt {
            process_id: process.id(),
            file_descriptor: 6,
            local_socket: "[ffc7::1]:54321".to_string(),
            peer_socket: "ffc7INVALID:81".to_string(),
        });
        let stream_status3 = client.remote_enroll(stream_creation3).await.unwrap_err();
        assert_eq!(
            stream_status3.message(),
            "Peer socket string can not be parsed."
        );

        process.kill()?;
        process.wait()?;

        let file_creation2 = tonic::Request::new(LocalCt {
            process_id: process.id(),
            file_descriptor: 3,
            path: "bar.txt".to_string(),
        });
        let file_status2 = client.local_enroll(file_creation2).await.unwrap_err();
        assert_eq!(
            file_status2.message(),
            format!("Process {} not found.", process.id())
        );

        let stream_creation4 = tonic::Request::new(RemoteCt {
            process_id: process.id(),
            file_descriptor: 4,
            local_socket: "[ffc7::1]:54321".to_string(),
            peer_socket: "ffc7::2:8081".to_string(),
        });
        let stream_status4 = client.remote_enroll(stream_creation4).await.unwrap_err();
        assert_eq!(
            stream_status4.message(),
            format!("Process {} not found.", process.id())
        );

        Ok(())
    }

    #[tokio::test]
    async fn unit_p2m_io_request_dead_process() -> Result<(), Box<dyn std::error::Error>> {
        let (sender, receiver) = mpsc::channel(32);
        tokio::spawn(provenance_layer(receiver));
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

        process.kill()?;
        process.wait()?;

        // Write event
        let write_request = tonic::Request::new(IoInfo {
            process_id: process.id(),
            file_descriptor: 3,
            flow: Flow::Output.into(),
        });
        let write_status = client.io_request(write_request).await.unwrap_err();
        assert_eq!(
            write_status.message(),
            format!("Process {} not found.", process.id())
        );

        Ok(())
    }

    #[tokio::test]
    async fn unit_p2m_io_request_unsupported_flow_type() -> Result<(), Box<dyn std::error::Error>> {
        let (sender, receiver) = mpsc::channel(32);
        tokio::spawn(provenance_layer(receiver));
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
        let unsupported_request = tonic::Request::new(IoInfo {
            process_id: process.id(),
            file_descriptor: 3,
            flow: Flow::None.into(),
        });
        let status = client.io_request(unsupported_request).await.unwrap_err();
        assert_eq!(status.message(), "Unsupported Flow type.");
        process.kill()?;
        Ok(())
    }

    #[tokio::test]
    async fn unit_p2m_io_request_not_enrolled() -> Result<(), Box<dyn std::error::Error>> {
        let (sender, receiver) = mpsc::channel(32);
        tokio::spawn(provenance_layer(receiver));
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
            file_descriptor: 4,
            flow: Flow::Output.into(),
        });
        let write_status = client.io_request(write_request).await.unwrap_err();
        assert_eq!(
            write_status.message(),
            format!("Process {} has not enrolled FD 4.", process.id())
        );

        process.kill()?;
        Ok(())
    }

    #[tokio::test]
    async fn unit_p2m_io_report_not_enrolled() -> Result<(), Box<dyn std::error::Error>> {
        let (sender, receiver) = mpsc::channel(32);
        tokio::spawn(provenance_layer(receiver));
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
        let write_report = tonic::Request::new(IoResult {
            process_id: process.id(),
            file_descriptor: 4,
            grant_id: 0, // no impact for this test
            result: true,
        });
        let write_status = client.io_report(write_report).await.unwrap_err();
        assert_eq!(
            write_status.message(),
            format!("Process {} has not enrolled FD 4.", process.id())
        );

        process.kill()?;
        Ok(())
    }

    #[tokio::test]
    async fn unit_p2m_scenario_file_write() -> Result<(), Box<dyn std::error::Error>> {
        let (sender, receiver) = mpsc::channel(32);
        tokio::spawn(provenance_layer(receiver));
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
    async fn unit_p2m_scenario_file_read() -> Result<(), Box<dyn std::error::Error>> {
        let (sender, receiver) = mpsc::channel(32);
        tokio::spawn(provenance_layer(receiver));
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
    async fn unit_p2m_scenario_stream_read() -> Result<(), Box<dyn std::error::Error>> {
        let (sender, receiver) = mpsc::channel(32);
        tokio::spawn(provenance_layer(receiver));
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
