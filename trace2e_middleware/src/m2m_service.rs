//! Middleware to Middleware gRPC Service.
use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use crate::{
    identifier::Identifier,
    labels::ComplianceLabel,
    traceability::{TraceabilityRequest, TraceabilityResponse},
};
use tokio::sync::{mpsc, oneshot, Mutex};
use tonic::{Request, Response, Status};

pub mod m2m {
    tonic::include_proto!("m2m_api");
    pub const M2M_DESCRIPTOR_SET: &[u8] = include_bytes!("../../target/m2m_descriptor.bin");
}

pub struct M2mService {
    synced_streams: Arc<
        Mutex<
            HashMap<
                Identifier,
                oneshot::Sender<(Vec<ComplianceLabel>, oneshot::Sender<TraceabilityResponse>)>,
            >,
        >,
    >,
    provenance: mpsc::Sender<TraceabilityRequest>,
}

impl M2mService {
    pub fn new(provenance_layer: mpsc::Sender<TraceabilityRequest>) -> Self {
        M2mService {
            synced_streams: Arc::new(Mutex::new(HashMap::new())),
            provenance: provenance_layer,
        }
    }
}

#[tonic::async_trait]
impl m2m::m2m_server::M2m for M2mService {
    async fn reserve(
        &self,
        request: Request<m2m::Stream>,
    ) -> Result<Response<m2m::Labels>, Status> {
        let r = request.into_inner();

        // Check consistency of the provided sockets
        let local_socket = match r.local_socket.clone().parse::<SocketAddr>() {
            Ok(socket) => socket,
            Err(_) => {
                return Err(Status::invalid_argument(format!(
                    "Local socket string can not be parsed."
                )));
            }
        };
        let peer_socket = match r.peer_socket.clone().parse::<SocketAddr>() {
            Ok(socket) => socket,
            Err(_) => {
                return Err(Status::invalid_argument(format!(
                    "Peer socket string can not be parsed."
                )));
            }
        };
        let stream_id = Identifier::new_stream(peer_socket, local_socket);
        let (tx, rx) = oneshot::channel();
        let _ = self
            .provenance
            .send(TraceabilityRequest::SyncStream(stream_id.clone(), tx))
            .await;
        match rx.await.unwrap() {
            TraceabilityResponse::WaitingSync(peer_stream_labels, provenance_sync_channel) => {
                self.synced_streams
                    .lock()
                    .await
                    .insert(stream_id, provenance_sync_channel);
                Ok(Response::new(peer_stream_labels.into()))
            }
            TraceabilityResponse::Error(e) => Err(Status::from_error(Box::new(e))),
            _ => unreachable!(),
        }
    }

    async fn sync_provenance(
        &self,
        request: Request<m2m::StreamProv>,
    ) -> Result<Response<m2m::Ack>, Status> {
        let r = request.into_inner();

        // Check consistency of the provided sockets
        let local_socket = match r.local_socket.clone().parse::<SocketAddr>() {
            Ok(socket) => socket,
            Err(_) => {
                return Err(Status::invalid_argument(format!(
                    "Local socket string can not be parsed."
                )));
            }
        };
        let peer_socket = match r.peer_socket.clone().parse::<SocketAddr>() {
            Ok(socket) => socket,
            Err(_) => {
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
                TraceabilityResponse::Recorded => Ok(Response::new(m2m::Ack {})),
                TraceabilityResponse::Error(e) => Err(Status::from_error(Box::new(e))),
                _ => unreachable!(),
            }
        } else {
            Err(Status::failed_precondition(format!(
                "{:?} is not synced.",
                stream_id
            )))
        }
    }
}
