//! Middleware to Middleware gRPC Service.
use std::net::SocketAddr;

use crate::{
    identifier::Identifier,
    provenance::{ProvenanceAction, ProvenanceResult},
};
use m2m::{m2m_server::M2m, Ack, StreamProv};
use tokio::sync::{mpsc, oneshot};
use tonic::{Request, Response, Status};

pub mod m2m {
    tonic::include_proto!("m2m_api");
    pub const M2M_DESCRIPTOR_SET: &[u8] = include_bytes!("../target/m2m_descriptor.bin");
}

pub struct M2mService {
    provenance: mpsc::Sender<ProvenanceAction>,
}

impl M2mService {
    pub fn new(provenance_layer: mpsc::Sender<ProvenanceAction>) -> Self {
        M2mService {
            provenance: provenance_layer,
        }
    }
}

#[tonic::async_trait]
impl M2m for M2mService {
    async fn sync_provenance(&self, request: Request<StreamProv>) -> Result<Response<Ack>, Status> {
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

        let provenance: Vec<Identifier> = r.provenance.into_iter().map(Identifier::from).collect();

        let (tx, rx) = oneshot::channel();
        let _ = self
            .provenance
            .send(ProvenanceAction::RemoteSync(
                Identifier::new_stream(peer_socket, local_socket),
                provenance,
                tx,
            ))
            .await;
        match rx.await.unwrap() {
            ProvenanceResult::Recorded => Ok(Response::new(Ack {})),
            ProvenanceResult::Error(e) => Err(Status::from_error(Box::new(e))),
            _ => unreachable!(),
        }
    }
}
