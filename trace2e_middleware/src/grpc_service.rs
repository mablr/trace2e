//! Unified Trace2e Middleware Service implementation.
use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use crate::{
    identifier::Identifier,
    labels::ComplianceLabel,
    traceability::{TraceabilityClient, TraceabilityResponse},
};
use procfs::process::Process;
use std::path::PathBuf;
use tokio::sync::{oneshot, Mutex, RwLock};
use tonic::{Request, Response, Status};
use tracing::{debug, error, info};

pub mod proto {
    tonic::include_proto!("trace2e");
    pub const MIDDLEWARE_DESCRIPTOR_SET: &[u8] = include_bytes!("../trace2e_descriptor.bin");
}

pub struct Trace2eService {
    identifiers_map: Arc<RwLock<HashMap<(u32, i32), Identifier>>>,
    synced_streams: Arc<
        Mutex<
            HashMap<
                Identifier,
                oneshot::Sender<(Vec<ComplianceLabel>, oneshot::Sender<TraceabilityResponse>)>,
            >,
        >,
    >,
    traceability: TraceabilityClient,
}

impl Trace2eService {
    pub fn new(traceability: TraceabilityClient) -> Self {
        Trace2eService {
            identifiers_map: Arc::new(RwLock::new(HashMap::new())),
            synced_streams: Arc::new(Mutex::new(HashMap::new())),
            traceability,
        }
    }
}

#[tonic::async_trait]
impl proto::trace2e_server::Trace2e for Trace2eService {
    // P2M operations
    async fn p2m_local_enroll(
        &self,
        request: Request<proto::LocalCt>,
    ) -> Result<Response<proto::Ack>, Status> {
        let r = request.into_inner();
        info!(
            "[P2M] ->M local_enroll (PID: {}, FD: {}, Path: {})",
            r.process_id.clone(),
            r.file_descriptor.clone(),
            r.path.clone()
        );

        let process_identifier = match Process::new(r.process_id.try_into().unwrap()) {
            Ok(p) => {
                let process_starttime = match p.stat() {
                    Ok(stat) => Ok(stat.starttime),
                    Err(_) => Err(Status::not_found(format!(
                        "Process {} stat failed.",
                        r.process_id
                    ))),
                };
                let process_exe_path = p
                    .exe()
                    .unwrap_or(PathBuf::default())
                    .to_str()
                    .unwrap_or("")
                    .to_string();
                Identifier::new_process(
                    r.process_id.try_into().unwrap(),
                    process_starttime?,
                    process_exe_path,
                )
            }
            Err(_) => {
                error!("Process {} not found.", r.process_id);
                return Err(Status::not_found(format!(
                    "Process {} not found.",
                    r.process_id
                )));
            }
        };

        let resource_identifier = Identifier::new_file(r.path.clone());

        let _ = self.traceability.register_container(process_identifier).await.map_err(|e| {
            error!("{}", e);
            Status::from_error(Box::new(e))
        })?;

        let _ = self.traceability.register_container(resource_identifier.clone()).await.map_err(|e| {
            error!("{}", e);
            Status::from_error(Box::new(e))
        })?;

        let mut identifiers_map = self.identifiers_map.write().await;
        identifiers_map.insert((r.process_id, r.file_descriptor), resource_identifier);

        info!(
            "[P2M] <-M local_enroll (PID: {}, FD: {}, Path: {})",
            r.process_id, r.file_descriptor, r.path
        );

        Ok(Response::new(proto::Ack {}))
    }

    async fn p2m_remote_enroll(
        &self,
        request: Request<proto::RemoteCt>,
    ) -> Result<Response<proto::Ack>, Status> {
        let r = request.into_inner();
        info!(
            "[P2M] ->M remote_enroll (PID: {}, FD: {}, Stream: [{}-{}])",
            r.process_id.clone(),
            r.file_descriptor.clone(),
            r.local_socket.clone(),
            r.peer_socket.clone()
        );

        let process_identifier = match Process::new(r.process_id.try_into().unwrap()) {
            Ok(p) => {
                let process_starttime = match p.stat() {
                    Ok(stat) => Ok(stat.starttime),
                    Err(_) => Err(Status::not_found(format!(
                        "Process {} stat failed.",
                        r.process_id
                    ))),
                };
                let process_exe_path = p
                    .exe()
                    .unwrap_or(PathBuf::default())
                    .to_str()
                    .unwrap_or("")
                    .to_string();
                Identifier::new_process(
                    r.process_id.try_into().unwrap(),
                    process_starttime?,
                    process_exe_path,
                )
            }
            Err(_) => {
                error!("Process {} not found.", r.process_id);
                return Err(Status::not_found(format!(
                    "Process {} not found.",
                    r.process_id
                )));
            }
        };

        let local_socket = match r.local_socket.clone().parse::<SocketAddr>() {
            Ok(socket) => socket,
            Err(_) => {
                error!("Local socket string can not be parsed.");
                return Err(Status::invalid_argument(format!(
                    "Local socket string can not be parsed."
                )));
            }
        };
        let peer_socket = match r.peer_socket.clone().parse::<SocketAddr>() {
            Ok(socket) => socket,
            Err(_) => {
                error!("Peer socket string can not be parsed.");
                return Err(Status::invalid_argument(format!(
                    "Peer socket string can not be parsed."
                )));
            }
        };

        let resource_identifier = Identifier::new_stream(local_socket, peer_socket);

        let _ = self.traceability.register_container(process_identifier).await.map_err(|e| {
            error!("{}", e);
            Status::from_error(Box::new(e))
        })?;

        let _ = self.traceability.register_container(resource_identifier.clone()).await.map_err(|e| {
            error!("{}", e);
            Status::from_error(Box::new(e))
        })?;

        let mut identifiers_map = self.identifiers_map.write().await;
        identifiers_map.insert((r.process_id, r.file_descriptor), resource_identifier);

        info!(
            "[P2M] <-M remote_enroll (PID: {}, FD: {}, Stream: [{}-{}])",
            r.process_id, r.file_descriptor, r.local_socket, r.peer_socket
        );

        Ok(Response::new(proto::Ack {}))
    }

    async fn p2m_io_request(
        &self,
        request: Request<proto::IoInfo>,
    ) -> Result<Response<proto::Grant>, Status> {
        let r = request.into_inner();
        info!(
            "[P2M] ->M io_request (PID: {}, FD: {}, Flow: {})",
            r.process_id, r.file_descriptor, r.flow
        );

        let process_identifier = match Process::new(r.process_id.try_into().unwrap()) {
            Ok(p) => {
                let process_starttime = match p.stat() {
                    Ok(stat) => Ok(stat.starttime),
                    Err(_) => Err(Status::not_found(format!(
                        "Process {} stat failed.",
                        r.process_id
                    ))),
                };
                let process_exe_path = p
                    .exe()
                    .unwrap_or(PathBuf::default())
                    .to_str()
                    .unwrap_or("")
                    .to_string();
                Identifier::new_process(
                    r.process_id.try_into().unwrap(),
                    process_starttime?,
                    process_exe_path,
                )
            }
            Err(_) => {
                error!("Process {} not found.", r.process_id);
                return Err(Status::not_found(format!(
                    "Process {} not found.",
                    r.process_id
                )));
            }
        };

        if let Some(resource_identifier) = self
            .identifiers_map
            .read()
            .await
            .get(&(r.process_id, r.file_descriptor))
            .cloned()
        {
            let output = match r.flow {
                flow_type if flow_type == proto::Flow::Output as i32 => true,
                flow_type if flow_type == proto::Flow::Input as i32 => false,
                _ => return Err(Status::invalid_argument("Unsupported Flow type.")),
            };
            match self.traceability.declare_flow(
                process_identifier.clone(),
                resource_identifier.clone(),
                output,
            ).await {
                Ok(grant_id) => {
                    info!(
                        "[P2M] <-M io_request (PID: {}, FD: {}, Flow: {}, grant_id: {})",
                        r.process_id, r.file_descriptor, r.flow, grant_id
                    );
                    Ok(Response::new(proto::Grant { id: grant_id }))
                }
                Err(e) => {
                    error!("{}", e);
                    Err(Status::from_error(Box::new(e)))
                }
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

    async fn p2m_io_report(
        &self,
        request: Request<proto::IoResult>,
    ) -> Result<Response<proto::Ack>, Status> {
        let r = request.into_inner();
        info!(
            "[P2M] ->M io_report (PID: {}, FD: {}, grant_id: {}, result: {})",
            r.process_id, r.file_descriptor, r.grant_id, r.result
        );

        if let Some(_resource_identifier) = self
            .identifiers_map
            .read()
            .await
            .get(&(r.process_id, r.file_descriptor))
            .cloned()
        {
            match self.traceability.record_flow(r.grant_id).await {
                Ok(()) => {
                    info!(
                        "[P2M] <-M io_report (PID: {}, FD: {}, grant_id: {}, result: {})",
                        r.process_id, r.file_descriptor, r.grant_id, r.result
                    );
                    Ok(Response::new(proto::Ack {}))
                }
                Err(e) => {
                    error!("{}", e);
                    Err(Status::from_error(Box::new(e)))
                }
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

    // M2M operations
    async fn m2m_reserve(
        &self,
        request: Request<proto::Stream>,
    ) -> Result<Response<proto::Labels>, Status> {
        let r = request.into_inner();
        info!(
            "[M2M] ->RM reserve (Stream: [{}-{}])",
            r.peer_socket.clone(),
            r.local_socket.clone()
        );

        let local_socket = match r.local_socket.clone().parse::<SocketAddr>() {
            Ok(socket) => socket,
            Err(_) => {
                error!("Local socket string can not be parsed.");
                return Err(Status::invalid_argument(format!(
                    "Local socket string can not be parsed."
                )));
            }
        };
        let peer_socket = match r.peer_socket.clone().parse::<SocketAddr>() {
            Ok(socket) => socket,
            Err(_) => {
                error!("Peer socket string can not be parsed.");
                return Err(Status::invalid_argument(format!(
                    "Peer socket string can not be parsed."
                )));
            }
        };
        let stream_id = Identifier::new_stream(peer_socket, local_socket);
        match self.traceability.sync_stream(stream_id.clone()).await {
            Ok((peer_stream_labels, provenance_sync_channel)) => {
                self.synced_streams
                    .lock()
                    .await
                    .insert(stream_id.clone(), provenance_sync_channel);
                debug!(
                    "[M2M] Stream: [{}-{}] -> WaitingSync",
                    r.peer_socket.clone(),
                    r.local_socket.clone()
                );
                info!(
                    "[M2M] <-RM reserve (Stream: [{}-{}])",
                    r.peer_socket.clone(),
                    r.local_socket.clone()
                );
                Ok(Response::new(peer_stream_labels.into()))
            }
            Err(e) => {
                error!("{:?}", e);
                Err(Status::from_error(Box::new(e)))
            }
        }
    }

    async fn m2m_sync_provenance(
        &self,
        request: Request<proto::StreamProv>,
    ) -> Result<Response<proto::Ack>, Status> {
        let r = request.into_inner();
        info!(
            "[M2M] ->RM sync_prov (Stream: [{}-{}])",
            r.peer_socket.clone(),
            r.local_socket.clone()
        );

        let local_socket = match r.local_socket.clone().parse::<SocketAddr>() {
            Ok(socket) => socket,
            Err(_) => {
                error!("Local socket string can not be parsed.");
                return Err(Status::invalid_argument(format!(
                    "Local socket string can not be parsed."
                )));
            }
        };
        let peer_socket = match r.peer_socket.clone().parse::<SocketAddr>() {
            Ok(socket) => socket,
            Err(_) => {
                error!("Peer socket string can not be parsed.");
                return Err(Status::invalid_argument(format!(
                    "Peer socket string can not be parsed."
                )));
            }
        };
        let stream_id = Identifier::new_stream(peer_socket, local_socket);

        if let Some(provenance_sync_channel) = self.synced_streams.lock().await.remove(&stream_id) {
            let provenance = r
                .provenance
                .into_iter()
                .map(ComplianceLabel::from)
                .collect();

            let (tx, rx) = oneshot::channel();
            let _ = provenance_sync_channel.send((provenance, tx));
            match rx.await.unwrap() {
                TraceabilityResponse::Recorded => {
                    info!(
                        "[M2M] <-RM sync_prov (Stream: [{}-{}])",
                        r.peer_socket.clone(),
                        r.local_socket.clone()
                    );
                    Ok(Response::new(proto::Ack {}))
                }
                TraceabilityResponse::Error(e) => {
                    error!("{:?}", e);
                    Err(Status::from_error(Box::new(e)))
                }
                _ => unreachable!(),
            }
        } else {
            error!("{:?} is not synced.", stream_id);
            Err(Status::failed_precondition(format!(
                "{:?} is not synced.",
                stream_id
            )))
        }
    }

    // User operations
    async fn user_print_db(
        &self,
        _request: Request<proto::Req>,
    ) -> Result<Response<proto::Ack>, Status> {
        match self.traceability.print_provenance().await {
            Ok(_) => Ok(Response::new(proto::Ack {})),
            Err(e) => Err(Status::from_error(Box::new(e))),
        }
    }

    async fn user_enable_local_confidentiality(
        &self,
        request: Request<proto::Resource>,
    ) -> Result<Response<proto::Ack>, Status> {
        let r = request.into_inner();
        match self.traceability.set_compliance_label(r.into(), Some(true), None).await {
            Ok(_) => Ok(Response::new(proto::Ack {})),
            Err(e) => Err(Status::from_error(Box::new(e))),
        }
    }

    async fn user_enable_local_integrity(
        &self,
        request: Request<proto::Resource>,
    ) -> Result<Response<proto::Ack>, Status> {
        let r = request.into_inner();
        match self.traceability.set_compliance_label(r.into(), None, Some(true)).await {
            Ok(_) => Ok(Response::new(proto::Ack {})),
            Err(e) => Err(Status::from_error(Box::new(e))),
        }
    }

    async fn user_disable_local_confidentiality(
        &self,
        request: Request<proto::Resource>,
    ) -> Result<Response<proto::Ack>, Status> {
        let r = request.into_inner();
        match self.traceability.set_compliance_label(r.into(), Some(false), None).await {
            Ok(_) => Ok(Response::new(proto::Ack {})),
            Err(e) => Err(Status::from_error(Box::new(e))),
        }
    }

    async fn user_disable_local_integrity(
        &self,
        request: Request<proto::Resource>,
    ) -> Result<Response<proto::Ack>, Status> {
        let r = request.into_inner();
        match self.traceability.set_compliance_label(r.into(), None, Some(false)).await {
            Ok(_) => Ok(Response::new(proto::Ack {})),
            Err(e) => Err(Status::from_error(Box::new(e))),
        }
    }
} 