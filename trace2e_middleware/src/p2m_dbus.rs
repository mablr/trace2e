//! Processes to Middleware gRPC Service.

use crate::{
    identifier::Identifier, p2m_service::p2m::Flow, traceability::TraceabilityClient
};
use procfs::process::Process;
use std::{collections::HashMap, path::PathBuf};
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::RwLock;
use tracing::{error, info};
use zbus::{fdo::Error, fdo::Result, interface};

pub struct P2mDbus {
    identifiers_map: Arc<RwLock<HashMap<(u32, i32), Identifier>>>,
    traceability: TraceabilityClient,
}

impl P2mDbus {
    pub fn new(traceability: TraceabilityClient) -> Self {
        P2mDbus {
            identifiers_map: Arc::new(RwLock::new(HashMap::new())),
            traceability,
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
            "[P2M-DBus] ->M local_enroll (PID: {}, FD: {}, Path: {})",
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

        let _ = self.traceability.register_container(process_identifier).await.map_err(|e| {
            error!("{}", e);
            Error::Failed(e.to_string())
        })?;

        let _ = self.traceability.register_container(resource_identifier.clone()).await.map_err(|e| {
            error!("{}", e);
            Error::Failed(e.to_string())
        })?;

        let mut identifiers_map = self.identifiers_map.write().await;
        identifiers_map.insert((process_id, file_descriptor), resource_identifier);

        info!(
            "[P2M-DBus] <-M local_enroll (PID: {}, FD: {}, Path: {})",
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
            "[P2M-DBus] ->M remote_enroll (PID: {}, FD: {}, Stream: [{}-{}])",
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

        let _ = self.traceability.register_container(process_identifier).await.map_err(|e| {
            error!("{}", e);
            Error::Failed(e.to_string())
        })?;

        let _ = self.traceability.register_container(resource_identifier.clone()).await.map_err(|e| {
            error!("{}", e);
            Error::Failed(e.to_string())
        })?;

        let mut identifiers_map = self.identifiers_map.write().await;
        identifiers_map.insert((process_id, file_descriptor), resource_identifier);

        info!(
            "[P2M-DBus] <-M remote_enroll (PID: {}, FD: {}, Stream: [{}-{}])",
            process_id, file_descriptor, local_socket, peer_socket
        );

        Ok(())
    }

    async fn io_request(&self, process_id: u32, file_descriptor: i32, flow: i32) -> Result<u64> {
        info!(
            "[P2M-DBus] ->M io_request (PID: {}, FD: {}, Flow: {})",
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
            let output = match flow {
                flow_type if flow_type == Flow::Output as i32 => true,
                flow_type if flow_type == Flow::Input as i32 => false,
                _ => return Err(Error::InvalidArgs("Unsupported Flow type.".to_string())),
            };
            match self.traceability.declare_flow(
                process_identifier.clone(),
                resource_identifier.clone(),
                output,
            ).await {
                Ok(grant_id) => {
                    info!(
                        "[P2M-DBus] <-M io_request (PID: {}, FD: {}, Flow: {}, grant_id: {})",
                        process_id, file_descriptor, flow, grant_id,
                    );
                    Ok(grant_id)
                }
                Err(e) => {
                    error!("{}", e);
                    return Err(Error::Failed(e.to_string()));
                }
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

    async fn io_report(
        &self,
        process_id: u32,
        file_descriptor: i32,
        grant_id: u64,
        result: bool,
    ) -> Result<()> {
        info!(
            "[P2M-DBus] ->M io_report (PID: {}, FD: {}, grant_id: {}, result: {})",
            process_id, file_descriptor, grant_id, result
        );

        if let Some(_resource_identifier) = self
            .identifiers_map
            .read()
            .await
            .get(&(process_id, file_descriptor))
            .cloned()
        {
            match self.traceability.record_flow(grant_id).await {
                Ok(()) => {
                    info!(
                        "[P2M-DBus] <-M io_report (PID: {}, FD: {}, grant_id: {}, result: {})",
                        process_id, file_descriptor, grant_id, result
                    );

                    Ok(())
                }
                Err(e) => {
                    error!("{}", e);
                    return Err(Error::Failed(e.to_string()));
                }
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
