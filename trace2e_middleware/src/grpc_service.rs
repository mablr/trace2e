//! gRPC service for the Trace2e middleware.
use tonic::{Request, Response, Status};

pub mod proto {
    tonic::include_proto!("trace2e");
    pub const MIDDLEWARE_DESCRIPTOR_SET: &[u8] = include_bytes!("../trace2e_descriptor.bin");
}

#[derive(Default)]
pub struct Trace2eService;

#[tonic::async_trait]
impl proto::trace2e_server::Trace2e for Trace2eService {
    // P2M operations
    async fn p2m_local_enroll(
        &self,
        _request: Request<proto::LocalCt>,
    ) -> Result<Response<proto::Ack>, Status> {
        Err(Status::unimplemented("Not implemented."))
    }

    async fn p2m_remote_enroll(
        &self,
        _request: Request<proto::RemoteCt>,
    ) -> Result<Response<proto::Ack>, Status> {
        Err(Status::unimplemented("Not implemented."))
    }

    async fn p2m_io_request(
        &self,
        _request: Request<proto::IoInfo>,
    ) -> Result<Response<proto::Grant>, Status> {
        Err(Status::unimplemented("Not implemented."))
    }

    async fn p2m_io_report(
        &self,
        _request: Request<proto::IoResult>,
    ) -> Result<Response<proto::Ack>, Status> {
        Err(Status::unimplemented("Not implemented."))
    }

    // M2M operations
    async fn m2m_reserve(
        &self,
        _request: Request<proto::Stream>,
    ) -> Result<Response<proto::Labels>, Status> {
        Err(Status::unimplemented("Not implemented."))
    }

    async fn m2m_sync_provenance(
        &self,
        _request: Request<proto::StreamProv>,
    ) -> Result<Response<proto::Ack>, Status> {
        Err(Status::unimplemented("Not implemented."))
    }

    // User operations
    async fn user_print_db(
        &self,
        __request: Request<proto::Req>,
    ) -> Result<Response<proto::Ack>, Status> {
        Err(Status::unimplemented("Not implemented."))
    }

    async fn user_enable_local_confidentiality(
        &self,
        _request: Request<proto::Resource>,
    ) -> Result<Response<proto::Ack>, Status> {
        Err(Status::unimplemented("Not implemented."))
    }

    async fn user_enable_local_integrity(
        &self,
        _request: Request<proto::Resource>,
    ) -> Result<Response<proto::Ack>, Status> {
        Err(Status::unimplemented("Not implemented."))
    }

    async fn user_disable_local_confidentiality(
        &self,
        _request: Request<proto::Resource>,
    ) -> Result<Response<proto::Ack>, Status> {
        Err(Status::unimplemented("Not implemented."))
    }

    async fn user_disable_local_integrity(
        &self,
        _request: Request<proto::Resource>,
    ) -> Result<Response<proto::Ack>, Status> {
        Err(Status::unimplemented("Not implemented."))
    }
}
