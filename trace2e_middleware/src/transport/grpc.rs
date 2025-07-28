//! gRPC service for the Trace2e middleware.
use tonic::{Request, Response, Status};
use tower::Service;

pub mod proto {
    tonic::include_proto!("trace2e");
    pub const MIDDLEWARE_DESCRIPTOR_SET: &[u8] = include_bytes!("../../trace2e_descriptor.bin");
}

use crate::traceability::{
    api::{P2mRequest, P2mResponse},
    error::TraceabilityError,
};

impl From<TraceabilityError> for Status {
    fn from(error: TraceabilityError) -> Self {
        Status::internal(error.to_string())
    }
}

pub struct Trace2eGrpcService<P2mApi> {
    p2m: P2mApi,
}

impl<P2mApi> Trace2eGrpcService<P2mApi> {
    pub fn new(p2m: P2mApi) -> Self {
        Self { p2m }
    }
}

#[tonic::async_trait]
impl<P2mApi> proto::trace2e_server::Trace2e for Trace2eGrpcService<P2mApi>
where
    P2mApi: Service<P2mRequest, Response = P2mResponse, Error = TraceabilityError>
        + Clone
        + Sync
        + Send
        + 'static,
    P2mApi::Future: Send,
{
    // P2M operations
    async fn p2m_local_enroll(
        &self,
        request: Request<proto::LocalCt>,
    ) -> Result<Response<proto::Ack>, Status> {
        let req = request.into_inner();
        let mut inner = self.p2m.clone();
        match inner
            .call(P2mRequest::LocalEnroll {
                pid: req.process_id,
                fd: req.file_descriptor,
                path: req.path,
            })
            .await?
        {
            P2mResponse::Ack => Ok(Response::new(proto::Ack {})),
            _ => Err(Status::internal("Internal traceability API error")),
        }
    }

    async fn p2m_remote_enroll(
        &self,
        request: Request<proto::RemoteCt>,
    ) -> Result<Response<proto::Ack>, Status> {
        let req = request.into_inner();
        let mut inner = self.p2m.clone();
        match inner
            .call(P2mRequest::RemoteEnroll {
                pid: req.process_id,
                fd: req.file_descriptor,
                local_socket: req.local_socket,
                peer_socket: req.peer_socket,
            })
            .await?
        {
            P2mResponse::Ack => Ok(Response::new(proto::Ack {})),
            _ => Err(Status::internal("Internal traceability API error")),
        }
    }

    async fn p2m_io_request(
        &self,
        request: Request<proto::IoInfo>,
    ) -> Result<Response<proto::Grant>, Status> {
        let req = request.into_inner();
        let mut inner = self.p2m.clone();
        match inner
            .call(P2mRequest::IoRequest {
                pid: req.process_id,
                fd: req.file_descriptor,
                output: req.flow == proto::Flow::Output as i32,
            })
            .await?
        {
            P2mResponse::Grant(id) => Ok(Response::new(proto::Grant { id: id.to_string() })),
            _ => Err(Status::internal("Internal traceability API error")),
        }
    }

    async fn p2m_io_report(
        &self,
        request: Request<proto::IoResult>,
    ) -> Result<Response<proto::Ack>, Status> {
        let req = request.into_inner();
        let mut inner = self.p2m.clone();
        match inner
            .call(P2mRequest::IoReport {
                pid: req.process_id,
                fd: req.file_descriptor,
                grant_id: req.grant_id.parse::<u128>().unwrap_or_default(),
                result: req.result,
            })
            .await?
        {
            P2mResponse::Ack => Ok(Response::new(proto::Ack {})),
            _ => Err(Status::internal("Internal traceability API error")),
        }
    }
}
