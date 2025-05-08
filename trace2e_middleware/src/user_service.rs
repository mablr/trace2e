//! User gRPC Service.
use crate::{
    traceability::TraceabilityClient,
    user_service::user::{user_server::User, Ack, Req, Resource},
};
use tonic::{Request, Response, Status};

pub mod user {
    tonic::include_proto!("user_api");
    pub const USER_DESCRIPTOR_SET: &[u8] = include_bytes!("../user_descriptor.bin");
}

pub struct UserService {
    traceability: TraceabilityClient,
}

impl UserService {
    pub fn new(traceability: TraceabilityClient) -> Self {
        UserService { traceability }
    }
}

#[tonic::async_trait]
impl User for UserService {
    async fn print_db(&self, _request: Request<Req>) -> Result<Response<Ack>, Status> {
        match self.traceability.print_provenance().await {
            Ok(_) => Ok(Response::new(Ack {})),
            Err(e) => Err(Status::from_error(Box::new(e))),
        }
    }

    async fn enable_local_confidentiality(
        &self,
        request: Request<Resource>,
    ) -> Result<Response<Ack>, Status> {
        let r = request.into_inner();

        match self.traceability.set_compliance_label(r.into(), Some(true), None).await {
            Ok(_) => Ok(Response::new(Ack {})),
            Err(e) => Err(Status::from_error(Box::new(e))),
        }
    }

    async fn enable_local_integrity(
        &self,
        request: Request<Resource>,
    ) -> Result<Response<Ack>, Status> {
        let r = request.into_inner();

        match self.traceability.set_compliance_label(r.into(), None, Some(true)).await {
            Ok(_) => Ok(Response::new(Ack {})),
            Err(e) => Err(Status::from_error(Box::new(e))),
        }
    }

    async fn disable_local_confidentiality(
        &self,
        request: Request<Resource>,
    ) -> Result<Response<Ack>, Status> {
        let r = request.into_inner();

        match self.traceability.set_compliance_label(r.into(), Some(false), None).await {
            Ok(_) => Ok(Response::new(Ack {})),
            Err(e) => Err(Status::from_error(Box::new(e))),
        }
    }

    async fn disable_local_integrity(
        &self,
        request: Request<Resource>,
    ) -> Result<Response<Ack>, Status> {
        let r = request.into_inner();

        match self.traceability.set_compliance_label(r.into(), None, Some(false)).await {
            Ok(_) => Ok(Response::new(Ack {})),
            Err(e) => Err(Status::from_error(Box::new(e))),
        }
    }
}
