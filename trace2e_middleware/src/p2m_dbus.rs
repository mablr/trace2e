//! Processes to Middleware gRPC Service.

use crate::{
    identifier::Identifier,
    traceability::{TraceabilityRequest, TraceabilityResponse},
};
use procfs::process::Process;
use std::{collections::HashMap, path::PathBuf};
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::{mpsc, oneshot, RwLock};
use tracing::{error, info};
use zbus::{fdo::Error, fdo::Result, interface};

#[derive(Debug)]
pub struct P2mDbus {
    identifiers_map: Arc<RwLock<HashMap<(u32, i32), Identifier>>>,
    provenance: mpsc::Sender<TraceabilityRequest>,
}

impl P2mDbus {
    pub fn new(traceability_server: mpsc::Sender<TraceabilityRequest>) -> Self {
        P2mDbus {
            identifiers_map: Arc::new(RwLock::new(HashMap::new())),
            provenance: traceability_server,
        }
    }
}

#[interface(name = "org.trace2e.P2m")]
impl P2mDbus {
    async fn local_enroll(
        &self,
        process_id: u32,
        file_descriptor: i32,
        path: String,
    ) -> Result<()> {
        info!(
            "[P2M] ->M local_enroll (PID: {}, FD: {}, Path: {})",
            process_id, file_descriptor, path
        );

        let process_identifier = match Process::new(process_id.try_into().unwrap()) {
            Ok(p) => {
                let process_starttime = match p.stat() {
                    Ok(stat) => Ok(stat.starttime),
                    Err(_) => Err(Error::Failed(format!(
                        "Process {} stat failed.",
                        process_id
                    ))),
                };
                let process_exe_path = p
                    .exe()
                    .unwrap_or(PathBuf::default())
                    .to_str()
                    .unwrap_or("")
                    .to_string();
                Identifier::new_process(
                    process_id.try_into().unwrap(),
                    process_starttime?,
                    process_exe_path,
                )
            }
            Err(_) => {
                error!("Process {} not found.", process_id);
                return Err(Error::Failed(format!("Process {} not found.", process_id)));
            }
        };

        let resource_identifier = Identifier::new_file(path.clone());

        let (tx, rx) = oneshot::channel();
        let _ = self
            .provenance
            .send(TraceabilityRequest::RegisterContainer(
                process_identifier,
                tx,
            ))
            .await;
        rx.await.unwrap();

        let (tx, rx) = oneshot::channel();
        let _ = self
            .provenance
            .send(TraceabilityRequest::RegisterContainer(
                resource_identifier.clone(),
                tx,
            ))
            .await;
        rx.await.unwrap();

        let mut identifiers_map = self.identifiers_map.write().await;
        identifiers_map.insert((process_id, file_descriptor), resource_identifier);

        info!(
            "[P2M] <-M local_enroll (PID: {}, FD: {}, Path: {})",
            process_id, file_descriptor, path
        );

        Ok(())
    }

    async fn remote_enroll(
        &self,
        process_id: u32,
        file_descriptor: i32,
        local_socket: String,
        peer_socket: String,
    ) -> Result<()> {
        info!(
            "[P2M] ->M remote_enroll (PID: {}, FD: {}, Stream: [{}-{}])",
            process_id, file_descriptor, local_socket, peer_socket
        );

        let process_identifier = match Process::new(process_id.try_into().unwrap()) {
            Ok(p) => {
                let process_starttime = match p.stat() {
                    Ok(stat) => Ok(stat.starttime),
                    Err(_) => Err(Error::Failed(format!(
                        "Process {} stat failed.",
                        process_id
                    ))),
                };
                let process_exe_path = p
                    .exe()
                    .unwrap_or(PathBuf::default())
                    .to_str()
                    .unwrap_or("")
                    .to_string();
                Identifier::new_process(
                    process_id.try_into().unwrap(),
                    process_starttime?,
                    process_exe_path,
                )
            }
            Err(_) => {
                error!("Process {} not found.", process_id);
                return Err(Error::Failed(format!("Process {} not found.", process_id)));
            }
        };

        // Check consistency of the provided sockets
        let local_socket = match local_socket.parse::<SocketAddr>() {
            Ok(socket) => socket,
            Err(_) => {
                error!("[{}] Local socket string can not be parsed.", local_socket);

                return Err(Error::InvalidArgs(
                    "Local socket string can not be parsed.".to_string(),
                ));
            }
        };
        let peer_socket = match peer_socket.parse::<SocketAddr>() {
            Ok(socket) => socket,
            Err(_) => {
                error!("[{}] Peer socket string can not be parsed.", peer_socket);

                return Err(Error::InvalidArgs(
                    "Peer socket string can not be parsed.".to_string(),
                ));
            }
        };

        let resource_identifier = Identifier::new_stream(local_socket, peer_socket);

        let (tx, rx) = oneshot::channel();
        let _ = self
            .provenance
            .send(TraceabilityRequest::RegisterContainer(
                process_identifier,
                tx,
            ))
            .await;
        rx.await.unwrap();

        let (tx, rx) = oneshot::channel();
        let _ = self
            .provenance
            .send(TraceabilityRequest::RegisterContainer(
                resource_identifier.clone(),
                tx,
            ))
            .await;
        rx.await.unwrap();

        let mut identifiers_map = self.identifiers_map.write().await;
        identifiers_map.insert((process_id, file_descriptor), resource_identifier);

        info!(
            "[P2M] <-M remote_enroll (PID: {}, FD: {}, Stream: [{}-{}])",
            process_id, file_descriptor, local_socket, peer_socket
        );

        Ok(())
    }

    async fn io_request(&self, process_id: u32, file_descriptor: i32, flow: bool) -> Result<u64> {
        info!(
            "[P2M] ->M io_request (PID: {}, FD: {}, Flow: {})",
            process_id, file_descriptor, flow
        );

        let process_identifier = match Process::new(process_id.try_into().unwrap()) {
            Ok(p) => {
                let process_starttime = match p.stat() {
                    Ok(stat) => Ok(stat.starttime),
                    Err(_) => Err(Error::Failed(format!(
                        "Process {} stat failed.",
                        process_id
                    ))),
                };
                let process_exe_path = p
                    .exe()
                    .unwrap_or(PathBuf::default())
                    .to_str()
                    .unwrap_or("")
                    .to_string();
                Identifier::new_process(
                    process_id.try_into().unwrap(),
                    process_starttime?,
                    process_exe_path,
                )
            }
            Err(_) => {
                error!("Process {} not found.", process_id);
                return Err(Error::Failed(format!("Process {} not found.", process_id)));
            }
        };

        if let Some(resource_identifier) = self
            .identifiers_map
            .read()
            .await
            .get(&(process_id, file_descriptor))
            .cloned()
        {
            let (tx, rx) = oneshot::channel();
            let _ = self
                .provenance
                .send(TraceabilityRequest::DeclareFlow(
                    process_identifier.clone(),
                    resource_identifier.clone(),
                    flow,
                    tx,
                ))
                .await;

            let grant_id = match rx.await.unwrap() {
                TraceabilityResponse::Declared(grant_id) => grant_id,
                TraceabilityResponse::Error(e) => {
                    error!("{}", e);
                    return Err(Error::Failed(e.to_string()));
                }
                _ => unreachable!(),
            };

            info!(
                "[P2M] <-M io_request (PID: {}, FD: {}, Flow: {}, grant_id: {})",
                process_id, file_descriptor, flow, grant_id,
            );

            Ok(grant_id)
        } else {
            error!(
                "Process {} has not enrolled FD {}.",
                process_id, file_descriptor
            );

            Err(Error::Failed(format!(
                "Process {} has not enrolled FD {}.",
                process_id, file_descriptor
            )))
        }
    }

    async fn io_report(
        &self,
        process_id: u32,
        file_descriptor: i32,
        grant_id: u64,
        result: bool,
    ) -> Result<()> {
        info!(
            "[P2M] ->M io_report (PID: {}, FD: {}, grant_id: {}, result: {})",
            process_id, file_descriptor, grant_id, result
        );

        if let Some(_resource_identifier) = self
            .identifiers_map
            .read()
            .await
            .get(&(process_id, file_descriptor))
            .cloned()
        {
            let (tx, rx) = oneshot::channel();
            let _ = self
                .provenance
                .send(TraceabilityRequest::RecordFlow(grant_id, tx))
                .await;
            match rx.await.unwrap() {
                TraceabilityResponse::Recorded => {
                    info!(
                        "[P2M] <-M io_report (PID: {}, FD: {}, grant_id: {}, result: {})",
                        process_id, file_descriptor, grant_id, result
                    );

                    Ok(())
                }
                TraceabilityResponse::Error(e) => {
                    error!("{}", e);
                    Err(Error::Failed(e.to_string()))
                }
                _ => unreachable!(),
            }
        } else {
            error!(
                "Process {} has not enrolled FD {}.",
                process_id, file_descriptor
            );

            Err(Error::Failed(format!(
                "Process {} has not enrolled FD {}.",
                process_id, file_descriptor
            )))
        }
    }
}
