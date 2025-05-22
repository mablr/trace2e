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
}
