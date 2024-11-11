//! User gRPC Service.
use crate::traceability::TraceabilityRequest;
use crate::user_service::user::{user_server::User, Ack, Req};
use tokio::sync::mpsc;
use tonic::{Request, Response, Status};

pub mod user {
    tonic::include_proto!("user_api");
    pub const USER_DESCRIPTOR_SET: &[u8] = include_bytes!("../../target/user_descriptor.bin");
}

pub struct UserService {
    traceability: mpsc::Sender<TraceabilityRequest>,
}

impl UserService {
    pub fn new(traceability: mpsc::Sender<TraceabilityRequest>) -> Self {
        UserService { traceability }
    }
}

#[tonic::async_trait]
impl User for UserService {
    async fn print_db(&self, _request: Request<Req>) -> Result<Response<Ack>, Status> {
        let _ = self
            .traceability
            .send(TraceabilityRequest::PrintProvenance)
            .await;
        Ok(Response::new(Ack {}))
    }
}
