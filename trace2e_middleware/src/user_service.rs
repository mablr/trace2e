//! User gRPC Service.
use crate::traceability::TraceabilityRequest;
use crate::user_service::user::{user_server::User, Ack, Req, Resource};
use tokio::sync::{mpsc, oneshot};
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

    async fn enable_local_confidentiality(
        &self,
        request: Request<Resource>,
    ) -> Result<Response<Ack>, Status> {
        let r = request.into_inner();

        let (tx, rx) = oneshot::channel();
        let _ = self
            .traceability
            .send(TraceabilityRequest::SetComplianceLabel(
                r.into(),
                Some(true),
                None,
                tx,
            ))
            .await;
        rx.await.unwrap();

        Ok(Response::new(Ack {}))
    }

    async fn enable_local_integrity(
        &self,
        request: Request<Resource>,
    ) -> Result<Response<Ack>, Status> {
        let r = request.into_inner();

        let (tx, rx) = oneshot::channel();
        let _ = self
            .traceability
            .send(TraceabilityRequest::SetComplianceLabel(
                r.into(),
                None,
                Some(true),
                tx,
            ))
            .await;
        rx.await.unwrap();

        Ok(Response::new(Ack {}))
    }

    async fn disable_local_confidentiality(
        &self,
        request: Request<Resource>,
    ) -> Result<Response<Ack>, Status> {
        let r = request.into_inner();

        let (tx, rx) = oneshot::channel();
        let _ = self
            .traceability
            .send(TraceabilityRequest::SetComplianceLabel(
                r.into(),
                Some(false),
                None,
                tx,
            ))
            .await;
        rx.await.unwrap();

        Ok(Response::new(Ack {}))
    }

    async fn disable_local_integrity(
        &self,
        request: Request<Resource>,
    ) -> Result<Response<Ack>, Status> {
        let r = request.into_inner();

        let (tx, rx) = oneshot::channel();
        let _ = self
            .traceability
            .send(TraceabilityRequest::SetComplianceLabel(
                r.into(),
                None,
                Some(false),
                tx,
            ))
            .await;
        rx.await.unwrap();

        Ok(Response::new(Ack {}))
    }
}
