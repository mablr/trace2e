//! gRPC service for the Trace2e middleware.
use std::{
    collections::{HashMap, HashSet},
    future::Future,
    pin::Pin,
    sync::Arc,
    task::Poll,
};
use tokio::sync::Mutex;
use tonic::{Request, Response, Status, transport::Channel};
use tower::Service;

pub const DEFAULT_GRPC_PORT: u16 = 50051;

pub mod proto {
    tonic::include_proto!("trace2e");
    pub const MIDDLEWARE_DESCRIPTOR_SET: &[u8] = include_bytes!("../../trace2e_descriptor.bin");
}

use crate::{
    traceability::{
        api::{M2mRequest, M2mResponse, P2mRequest, P2mResponse},
        error::TraceabilityError,
        layers::compliance::{ConfidentialityPolicy, Policy},
        naming::{Fd, File, Process, Resource, Stream},
    },
    transport::eval_remote_ip,
};

impl From<TraceabilityError> for Status {
    fn from(error: TraceabilityError) -> Self {
        Status::internal(error.to_string())
    }
}

#[derive(Default, Clone)]
pub struct M2mGrpc {
    connected_remotes:
        Arc<Mutex<HashMap<String, proto::trace2e_grpc_client::Trace2eGrpcClient<Channel>>>>,
}

impl M2mGrpc {
    async fn reconnect_remote(
        &self,
        remote_ip: String,
    ) -> Result<proto::trace2e_grpc_client::Trace2eGrpcClient<Channel>, TraceabilityError> {
        match proto::trace2e_grpc_client::Trace2eGrpcClient::connect(format!(
            "{remote_ip}:{DEFAULT_GRPC_PORT}"
        ))
        .await
        {
            Ok(client) => {
                self.connected_remotes
                    .lock()
                    .await
                    .insert(remote_ip, client.clone());
                Ok(client)
            }
            Err(_) => Err(TraceabilityError::TransportFailedToContactRemote(remote_ip)),
        }
    }

    async fn get_client(
        &self,
        remote_ip: String,
    ) -> Option<proto::trace2e_grpc_client::Trace2eGrpcClient<Channel>> {
        let connected_remotes = self.connected_remotes.lock().await;
        connected_remotes.get(&remote_ip).cloned()
    }

    async fn get_client_or_connect(
        &self,
        remote_ip: String,
    ) -> Result<proto::trace2e_grpc_client::Trace2eGrpcClient<Channel>, TraceabilityError> {
        match self.get_client(remote_ip.clone()).await {
            Some(client) => Ok(client),
            None => self.reconnect_remote(remote_ip).await,
        }
    }
}

impl Service<M2mRequest> for M2mGrpc {
    type Response = M2mResponse;
    type Error = TraceabilityError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: M2mRequest) -> Self::Future {
        let this = self.clone();
        Box::pin(async move {
            let Some(remote_ip) = eval_remote_ip(request.clone()) else {
                return Err(TraceabilityError::TransportFailedToEvaluateRemote);
            };

            match request {
                M2mRequest::GetConsistentCompliance {
                    source,
                    destination,
                } => {
                    // Get or connect to the remote client
                    let mut client = this.get_client_or_connect(remote_ip.clone()).await?;

                    // Create the protobuf request
                    let proto_req = proto::ConsistentCompliance {
                        source: Some(source.into()),
                        destination: Some(destination.into()),
                    };

                    // Make the gRPC call
                    let response = client
                        .m2m_consistent_compliance(Request::new(proto_req))
                        .await
                        .map_err(|_| TraceabilityError::TransportFailedToContactRemote(remote_ip))?
                        .into_inner();
                    Ok(M2mResponse::Compliance(
                        response
                            .policies
                            .into_iter()
                            .map(|policy| policy.into())
                            .collect(),
                    ))
                }
                M2mRequest::GetLooseCompliance { resources, .. } => {
                    // Get or connect to the remote client
                    let mut client = this.get_client_or_connect(remote_ip.clone()).await?;

                    // Create the protobuf request
                    let proto_req = proto::LooseCompliance {
                        resources: resources.into_iter().map(|r| r.into()).collect(),
                    };

                    // Make the gRPC call
                    let response = client
                        .m2m_loose_compliance(Request::new(proto_req))
                        .await
                        .map_err(|_| TraceabilityError::TransportFailedToContactRemote(remote_ip))?
                        .into_inner();
                    Ok(M2mResponse::Compliance(
                        response
                            .policies
                            .into_iter()
                            .map(|policy| policy.into())
                            .collect(),
                    ))
                }
                M2mRequest::ProvenanceUpdate {
                    source_prov,
                    destination,
                } => {
                    // Get or connect to the remote client
                    let mut client = this.get_client_or_connect(remote_ip.clone()).await?;

                    // Create the protobuf request
                    let proto_req = proto::ProvenanceUpdate {
                        source_prov: source_prov.into_iter().map(|s| s.into()).collect(),
                        destination: Some(destination.into()),
                    };

                    // Make the gRPC call
                    client
                        .m2m_provenance_update(Request::new(proto_req))
                        .await
                        .map_err(|_| {
                            TraceabilityError::TransportFailedToContactRemote(remote_ip)
                        })?;

                    Ok(M2mResponse::Ack)
                }
            }
        })
    }
}

pub struct Trace2eRouter<P2mApi, M2mApi> {
    p2m: P2mApi,
    m2m: M2mApi,
}

impl<P2mApi, M2mApi> Trace2eRouter<P2mApi, M2mApi> {
    pub fn new(p2m: P2mApi, m2m: M2mApi) -> Self {
        Self { p2m, m2m }
    }
}

#[tonic::async_trait]
impl<P2mApi, M2mApi> proto::trace2e_grpc_server::Trace2eGrpc for Trace2eRouter<P2mApi, M2mApi>
where
    P2mApi: Service<P2mRequest, Response = P2mResponse, Error = TraceabilityError>
        + Clone
        + Sync
        + Send
        + 'static,
    P2mApi::Future: Send,
    M2mApi: Service<M2mRequest, Response = M2mResponse, Error = TraceabilityError>
        + Clone
        + Sync
        + Send
        + 'static,
    M2mApi::Future: Send,
{
    // P2M operations
    async fn p2m_local_enroll(
        &self,
        request: Request<proto::LocalCt>,
    ) -> Result<Response<proto::Ack>, Status> {
        let req = request.into_inner();
        let mut p2m = self.p2m.clone();
        match p2m
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
        let mut p2m = self.p2m.clone();
        match p2m
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
        let mut p2m = self.p2m.clone();
        match p2m
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
        let mut p2m = self.p2m.clone();
        match p2m
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

    // M2M operations
    async fn m2m_consistent_compliance(
        &self,
        request: Request<proto::ConsistentCompliance>,
    ) -> Result<Response<proto::Compliance>, Status> {
        let req = request.into_inner();
        let mut m2m = self.m2m.clone();
        match m2m.call(req.into()).await? {
            M2mResponse::Compliance(policies) => Ok(Response::new(policies.into())),
            _ => Err(Status::internal("Internal traceability API error")),
        }
    }

    async fn m2m_loose_compliance(
        &self,
        request: Request<proto::LooseCompliance>,
    ) -> Result<Response<proto::Compliance>, Status> {
        let req = request.into_inner();
        let mut m2m = self.m2m.clone();
        match m2m.call(req.into()).await? {
            M2mResponse::Compliance(policies) => Ok(Response::new(policies.into())),
            _ => Err(Status::internal("Internal traceability API error")),
        }
    }

    async fn m2m_provenance_update(
        &self,
        request: Request<proto::ProvenanceUpdate>,
    ) -> Result<Response<proto::Ack>, Status> {
        let req = request.into_inner();
        let mut m2m = self.m2m.clone();
        match m2m.call(req.into()).await? {
            M2mResponse::Ack => Ok(Response::new(proto::Ack {})),
            _ => Err(Status::internal("Internal traceability API error")),
        }
    }
}

// Conversion trait implementations

impl From<proto::ConsistentCompliance> for M2mRequest {
    fn from(req: proto::ConsistentCompliance) -> Self {
        M2mRequest::GetConsistentCompliance {
            source: req.source.map(|s| s.into()).unwrap_or_default(),
            destination: req.destination.map(|d| d.into()).unwrap_or_default(),
        }
    }
}

impl From<proto::LooseCompliance> for M2mRequest {
    fn from(req: proto::LooseCompliance) -> Self {
        M2mRequest::GetLooseCompliance {
            authority_ip: String::new(), // Already routed so no authority IP needed
            resources: req.resources.into_iter().map(|r| r.into()).collect(),
        }
    }
}

impl From<proto::References> for (String, HashSet<Resource>) {
    fn from(references: proto::References) -> Self {
        (
            references.node,
            references.resources.into_iter().map(|r| r.into()).collect(),
        )
    }
}

impl From<proto::ProvenanceUpdate> for M2mRequest {
    fn from(req: proto::ProvenanceUpdate) -> Self {
        M2mRequest::ProvenanceUpdate {
            source_prov: req.source_prov.into_iter().map(|s| s.into()).collect(),
            destination: req.destination.map(|d| d.into()).unwrap_or_default(),
        }
    }
}

impl From<HashSet<Policy>> for proto::Compliance {
    fn from(policies: HashSet<Policy>) -> Self {
        proto::Compliance {
            policies: policies.into_iter().map(|policy| policy.into()).collect(),
        }
    }
}

impl From<(String, HashSet<Resource>)> for proto::References {
    fn from((node, resources): (String, HashSet<Resource>)) -> Self {
        proto::References {
            node,
            resources: resources.into_iter().map(|r| r.into()).collect(),
        }
    }
}

impl From<proto::Resource> for Resource {
    fn from(proto_resource: proto::Resource) -> Self {
        match proto_resource.resource {
            Some(proto::resource::Resource::Fd(fd)) => Resource::Fd(fd.into()),
            Some(proto::resource::Resource::Process(process)) => Resource::Process(process.into()),
            None => Resource::None,
        }
    }
}

impl From<Resource> for proto::Resource {
    fn from(resource: Resource) -> Self {
        match resource {
            Resource::Fd(fd) => proto::Resource {
                resource: Some(proto::resource::Resource::Fd(fd.into())),
            },
            Resource::Process(process) => proto::Resource {
                resource: Some(proto::resource::Resource::Process(process.into())),
            },
            Resource::None => proto::Resource { resource: None },
        }
    }
}

impl From<proto::Fd> for Fd {
    fn from(proto_fd: proto::Fd) -> Self {
        match proto_fd.fd {
            Some(proto::fd::Fd::File(file)) => Fd::File(file.into()),
            Some(proto::fd::Fd::Stream(stream)) => Fd::Stream(stream.into()),
            None => Fd::File(File {
                path: String::new(),
            }), // Default to empty file
        }
    }
}

impl From<Fd> for proto::Fd {
    fn from(fd: Fd) -> Self {
        match fd {
            Fd::File(file) => proto::Fd {
                fd: Some(proto::fd::Fd::File(file.into())),
            },
            Fd::Stream(stream) => proto::Fd {
                fd: Some(proto::fd::Fd::Stream(stream.into())),
            },
        }
    }
}

impl From<proto::File> for File {
    fn from(proto_file: proto::File) -> Self {
        File {
            path: proto_file.path,
        }
    }
}

impl From<File> for proto::File {
    fn from(file: File) -> Self {
        proto::File { path: file.path }
    }
}

impl From<proto::Stream> for Stream {
    fn from(proto_stream: proto::Stream) -> Self {
        Stream {
            local_socket: proto_stream.local_socket,
            peer_socket: proto_stream.peer_socket,
        }
    }
}

impl From<Stream> for proto::Stream {
    fn from(stream: Stream) -> Self {
        proto::Stream {
            local_socket: stream.local_socket,
            peer_socket: stream.peer_socket,
        }
    }
}

impl From<proto::Process> for Process {
    fn from(proto_process: proto::Process) -> Self {
        Process {
            pid: proto_process.pid,
            starttime: proto_process.starttime,
            exe_path: proto_process.exe_path,
        }
    }
}

impl From<Process> for proto::Process {
    fn from(process: Process) -> Self {
        proto::Process {
            pid: process.pid,
            starttime: process.starttime,
            exe_path: process.exe_path,
        }
    }
}

impl From<Policy> for proto::Policy {
    fn from(policy: Policy) -> Self {
        proto::Policy {
            confidentiality: match policy.confidentiality {
                ConfidentialityPolicy::Public => proto::Confidentiality::Public as i32,
                ConfidentialityPolicy::Secret => proto::Confidentiality::Secret as i32,
            },
            integrity: policy.integrity,
        }
    }
}

impl From<proto::Policy> for Policy {
    fn from(proto_policy: proto::Policy) -> Self {
        Policy {
            confidentiality: match proto_policy.confidentiality {
                x if x == proto::Confidentiality::Secret as i32 => ConfidentialityPolicy::Secret,
                _ => ConfidentialityPolicy::Public,
            },
            integrity: proto_policy.integrity,
        }
    }
}
