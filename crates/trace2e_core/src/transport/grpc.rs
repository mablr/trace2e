//! # gRPC Transport Implementation
//!
//! This module provides the gRPC-based transport layer for distributed traceability
//! operations in the trace2e framework. It implements both client and server
//! functionality for machine-to-machine (M2M) communication using Protocol Buffers
//! and the Tonic gRPC framework.
//!
//! ## Components
//!
//! - **M2mGrpc**: Client service for making outbound gRPC calls to remote middleware
//! - **Trace2eRouter**: Server implementation that routes incoming gRPC requests
//! - **Protocol Buffer Conversions**: Type conversions between internal and protobuf types
//!
//! ## Connection Management
//!
//! The gRPC client maintains a cache of connected remote clients to avoid
//! repeated connection overhead. Connections are established on-demand and
//! reused for subsequent requests to the same remote endpoint.
//!
//! ## Service Operations
//!
//! ### Process-to-Middleware (P2M)
//! - Local process enrollment (file descriptors)
//! - Remote process enrollment (network connections)
//! - I/O request authorization
//! - I/O operation reporting
//!
//! ### Machine-to-Machine (M2M)
//! - Destination compliance policy retrieval
//! - Source compliance policy retrieval
//! - Provenance information updates
//!
//! ## Protocol Buffer Integration
//!
//! The module includes comprehensive type conversions between the internal
//! trace2e types and their Protocol Buffer representations, ensuring seamless
//! serialization and deserialization across network boundaries.

use std::{
    collections::{HashMap, HashSet},
    future::Future,
    pin::Pin,
    sync::Arc,
    task::Poll,
};

use dashmap::DashMap;
use tonic::{Request, Response, Status, transport::Channel};
use tower::Service;

/// Default port for gRPC communication between trace2e middleware instances.
pub const DEFAULT_GRPC_PORT: u16 = 50051;

/// Protocol Buffer definitions and descriptor sets for the trace2e gRPC service.
pub mod proto {
    tonic::include_proto!("trace2e");
    pub mod primitives {
        tonic::include_proto!("trace2e.primitives");
    }
    pub mod messages {
        tonic::include_proto!("trace2e.messages");
    }

    /// Pre-compiled Protocol Buffer descriptor set for service reflection.
    pub const MIDDLEWARE_DESCRIPTOR_SET: &[u8] = include_bytes!("../../trace2e_descriptor.bin");
}

use crate::{
    traceability::{
        api::{M2mRequest, M2mResponse, P2mRequest, P2mResponse},
        core::compliance::{ConfidentialityPolicy, Policy},
        error::TraceabilityError,
        naming::{Fd, File, Process, Resource, Stream},
    },
    transport::eval_remote_ip,
};

/// Converts traceability errors to gRPC Status codes for wire transmission.
impl From<TraceabilityError> for Status {
    fn from(error: TraceabilityError) -> Self {
        Status::internal(error.to_string())
    }
}

/// gRPC client service for machine-to-machine communication.
///
/// `M2mGrpc` provides the client-side implementation for making gRPC calls to
/// remote trace2e middleware instances. It manages connection pooling and
/// handles the translation between internal M2M requests and gRPC protocol.
///
/// ## Connection Management
///
/// The service maintains a thread-safe cache of connected clients to avoid
/// connection overhead. Connections are established lazily when first needed
/// and reused for subsequent requests to the same remote endpoint.
///
/// ## Request Routing
///
/// The service automatically determines the target remote IP address from
/// the request payload and routes the call to the appropriate endpoint.
/// Network failures are reported as transport errors.
#[derive(Default, Clone)]
pub struct M2mGrpc {
    /// Cache of established gRPC client connections indexed by remote IP address.
    connected_remotes: Arc<DashMap<String, proto::trace2e_grpc_client::Trace2eGrpcClient<Channel>>>,
}

impl M2mGrpc {
    /// Establishes a new gRPC connection to a remote middleware instance.
    ///
    /// Creates a new client connection to the specified remote IP address
    /// using the default gRPC port. The connection is cached for future use.
    ///
    /// # Arguments
    ///
    /// * `remote_ip` - The IP address of the remote middleware instance
    ///
    /// # Returns
    ///
    /// A connected gRPC client, or an error if connection fails.
    ///
    /// # Errors
    ///
    /// Returns `TransportFailedToContactRemote` if the connection cannot be established.
    async fn connect_remote(
        &self,
        remote_ip: String,
    ) -> Result<proto::trace2e_grpc_client::Trace2eGrpcClient<Channel>, TraceabilityError> {
        match proto::trace2e_grpc_client::Trace2eGrpcClient::connect(format!(
            "{remote_ip}:{DEFAULT_GRPC_PORT}"
        ))
        .await
        {
            Ok(client) => {
                self.connected_remotes.insert(remote_ip, client.clone());
                Ok(client)
            }
            Err(_) => Err(TraceabilityError::TransportFailedToContactRemote(remote_ip)),
        }
    }

    /// Retrieves an existing cached gRPC client for the specified remote IP.
    ///
    /// # Arguments
    ///
    /// * `remote_ip` - The IP address to look up in the connection cache
    ///
    /// # Returns
    ///
    /// An existing client connection if available, None otherwise.
    async fn get_client(
        &self,
        remote_ip: String,
    ) -> Option<proto::trace2e_grpc_client::Trace2eGrpcClient<Channel>> {
        self.connected_remotes.get(&remote_ip).map(|c| c.to_owned())
    }

    /// Retrieves an existing client or establishes a new connection if needed.
    ///
    /// This method first checks the connection cache and returns an existing
    /// client if available. If no cached connection exists, it establishes
    /// a new connection and caches it for future use.
    ///
    /// # Arguments
    ///
    /// * `remote_ip` - The IP address of the target middleware instance
    ///
    /// # Returns
    ///
    /// A ready-to-use gRPC client connection.
    ///
    /// # Errors
    ///
    /// Returns `TransportFailedToContactRemote` if connection establishment fails.
    async fn get_client_or_connect(
        &self,
        remote_ip: String,
    ) -> Result<proto::trace2e_grpc_client::Trace2eGrpcClient<Channel>, TraceabilityError> {
        match self.get_client(remote_ip.clone()).await {
            Some(client) => Ok(client),
            None => self.connect_remote(remote_ip).await,
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
            let remote_ip = eval_remote_ip(request.clone())?;
            let mut client = this.get_client_or_connect(remote_ip.clone()).await?;
            match request {
                M2mRequest::GetDestinationCompliance { source, destination } => {
                    // Create the protobuf request
                    let proto_req = proto::messages::GetDestinationCompliance {
                        source: Some(source.into()),
                        destination: Some(destination.into()),
                    };

                    // Make the gRPC call
                    let response = client
                        .m2m_destination_compliance(Request::new(proto_req))
                        .await
                        .map_err(|_| TraceabilityError::TransportFailedToContactRemote(remote_ip))?
                        .into_inner();
                    Ok(M2mResponse::DestinationCompliance(
                        response.policy.map(|policy| policy.into()).unwrap_or_default(),
                    ))
                }
                M2mRequest::GetSourceCompliance { resources, .. } => {
                    // Create the protobuf request
                    let proto_req = proto::messages::GetSourceCompliance {
                        resources: resources.into_iter().map(|r| r.into()).collect(),
                    };

                    // Make the gRPC call
                    let response = client
                        .m2m_source_compliance(Request::new(proto_req))
                        .await
                        .map_err(|_| TraceabilityError::TransportFailedToContactRemote(remote_ip))?
                        .into_inner();
                    Ok(M2mResponse::SourceCompliance(
                        response
                            .policies
                            .into_iter()
                            .map(|policy| {
                                (
                                    policy.resource.map(|r| r.into()).unwrap_or_default(),
                                    policy.policy.map(|p| p.into()).unwrap_or_default(),
                                )
                            })
                            .collect(),
                    ))
                }
                M2mRequest::UpdateProvenance { source_prov, destination } => {
                    // Create the protobuf request
                    let proto_req = proto::messages::UpdateProvenance {
                        source_prov: source_prov.into_iter().map(|s| s.into()).collect(),
                        destination: Some(destination.into()),
                    };

                    // Make the gRPC call
                    client.m2m_update_provenance(Request::new(proto_req)).await.map_err(|_| {
                        TraceabilityError::TransportFailedToContactRemote(remote_ip)
                    })?;

                    Ok(M2mResponse::Ack)
                }
            }
        })
    }
}

/// gRPC server router that handles incoming requests and routes them to appropriate services.
///
/// `Trace2eRouter` implements the gRPC server-side logic by accepting incoming
/// requests and routing them to the appropriate process-to-middleware (P2M) or
/// machine-to-machine (M2M) service handlers.
///
/// ## Type Parameters
///
/// * `P2mApi` - Service handling process-to-middleware requests
/// * `M2mApi` - Service handling machine-to-machine requests
///
/// ## Request Routing
///
/// The router translates incoming Protocol Buffer requests to internal API
/// types, calls the appropriate service, and converts responses back to
/// Protocol Buffer format for transmission.
pub struct Trace2eRouter<P2mApi, M2mApi> {
    /// Process-to-middleware service handler.
    p2m: P2mApi,
    /// Machine-to-machine service handler.
    m2m: M2mApi,
}

impl<P2mApi, M2mApi> Trace2eRouter<P2mApi, M2mApi> {
    /// Creates a new router with the specified service handlers.
    ///
    /// # Arguments
    ///
    /// * `p2m` - Service for handling process-to-middleware requests
    /// * `m2m` - Service for handling machine-to-machine requests
    pub fn new(p2m: P2mApi, m2m: M2mApi) -> Self {
        Self { p2m, m2m }
    }
}

/// Implementation of the trace2e gRPC service protocol.
///
/// This implementation provides the server-side handlers for all gRPC endpoints
/// defined in the trace2e protocol. It handles both P2M (process-to-middleware)
/// and M2M (machine-to-machine) operations by delegating to the appropriate
/// internal service handlers.
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
    /// Handles local process enrollment requests.
    ///
    /// Registers a local file descriptor with the middleware for tracking.
    /// This is called when a process opens a local file or resource.
    async fn p2m_local_enroll(
        &self,
        request: Request<proto::messages::LocalCt>,
    ) -> Result<Response<proto::messages::Ack>, Status> {
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
            P2mResponse::Ack => Ok(Response::new(proto::messages::Ack {})),
            _ => Err(Status::internal("Internal traceability API error")),
        }
    }

    /// Handles remote process enrollment requests.
    ///
    /// Registers a network connection (socket) with the middleware for tracking.
    /// This is called when a process establishes a network connection.
    async fn p2m_remote_enroll(
        &self,
        request: Request<proto::messages::RemoteCt>,
    ) -> Result<Response<proto::messages::Ack>, Status> {
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
            P2mResponse::Ack => Ok(Response::new(proto::messages::Ack {})),
            _ => Err(Status::internal("Internal traceability API error")),
        }
    }

    /// Handles I/O authorization requests from processes.
    ///
    /// Evaluates whether a process is authorized to perform an I/O operation
    /// on a specific file descriptor. Returns a grant ID if authorized.
    async fn p2m_io_request(
        &self,
        request: Request<proto::messages::IoInfo>,
    ) -> Result<Response<proto::messages::Grant>, Status> {
        let req = request.into_inner();
        let mut p2m = self.p2m.clone();
        match p2m
            .call(P2mRequest::IoRequest {
                pid: req.process_id,
                fd: req.file_descriptor,
                output: req.flow == proto::primitives::Flow::Output as i32,
            })
            .await?
        {
            P2mResponse::Grant(id) => {
                Ok(Response::new(proto::messages::Grant { id: id.to_string() }))
            }
            _ => Err(Status::internal("Internal traceability API error")),
        }
    }

    /// Handles I/O operation completion reports from processes.
    ///
    /// Records the completion and result of an I/O operation that was
    /// previously authorized. This completes the audit trail for the operation.
    async fn p2m_io_report(
        &self,
        request: Request<proto::messages::IoResult>,
    ) -> Result<Response<proto::messages::Ack>, Status> {
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
            P2mResponse::Ack => Ok(Response::new(proto::messages::Ack {})),
            _ => Err(Status::internal("Internal traceability API error")),
        }
    }

    /// Handles destination compliance policy requests from remote middleware.
    ///
    /// Returns the compliance policy for a destination resource to enable
    /// remote middleware instances to evaluate flow authorization.
    async fn m2m_destination_compliance(
        &self,
        request: Request<proto::messages::GetDestinationCompliance>,
    ) -> Result<Response<proto::messages::DestinationCompliance>, Status> {
        let req = request.into_inner();
        let mut m2m = self.m2m.clone();
        match m2m.call(req.into()).await? {
            M2mResponse::DestinationCompliance(policy) => Ok(Response::new(policy.into())),
            _ => Err(Status::internal("Internal traceability API error")),
        }
    }

    /// Handles source compliance policy requests from remote middleware.
    ///
    /// Returns the compliance policies for a set of source resources to enable
    /// distributed flow evaluation across multiple middleware instances.
    async fn m2m_source_compliance(
        &self,
        request: Request<proto::messages::GetSourceCompliance>,
    ) -> Result<Response<proto::messages::SourceCompliance>, Status> {
        let req = request.into_inner();
        let mut m2m = self.m2m.clone();
        match m2m.call(req.into()).await? {
            M2mResponse::SourceCompliance(policies) => Ok(Response::new(policies.into())),
            _ => Err(Status::internal("Internal traceability API error")),
        }
    }

    /// Handles provenance update requests from remote middleware.
    ///
    /// Updates the provenance information for a destination resource based
    /// on data flows from remote sources. This maintains the audit trail
    /// across distributed operations.
    async fn m2m_update_provenance(
        &self,
        request: Request<proto::messages::UpdateProvenance>,
    ) -> Result<Response<proto::messages::Ack>, Status> {
        let req = request.into_inner();
        let mut m2m = self.m2m.clone();
        match m2m.call(req.into()).await? {
            M2mResponse::Ack => Ok(Response::new(proto::messages::Ack {})),
            _ => Err(Status::internal("Internal traceability API error")),
        }
    }
}

// Protocol Buffer type conversion implementations
//
// The following implementations provide bidirectional conversion between
// internal trace2e types and their Protocol Buffer representations,
// enabling seamless serialization for network transmission.

/// Converts Protocol Buffer GetDestinationCompliance request to internal M2M request.
impl From<proto::messages::GetDestinationCompliance> for M2mRequest {
    fn from(req: proto::messages::GetDestinationCompliance) -> Self {
        M2mRequest::GetDestinationCompliance {
            source: req.source.map(|s| s.into()).unwrap_or_default(),
            destination: req.destination.map(|d| d.into()).unwrap_or_default(),
        }
    }
}

/// Converts Protocol Buffer GetSourceCompliance request to internal M2M request.
impl From<proto::messages::GetSourceCompliance> for M2mRequest {
    fn from(req: proto::messages::GetSourceCompliance) -> Self {
        M2mRequest::GetSourceCompliance {
            authority_ip: String::new(), // Already routed so no authority IP needed
            resources: req.resources.into_iter().map(|r| r.into()).collect(),
        }
    }
}

/// Converts Protocol Buffer MappedPolicy to internal resource-policy tuple.
impl From<proto::primitives::MappedPolicy> for (Resource, Policy) {
    fn from(policy: proto::primitives::MappedPolicy) -> Self {
        (
            policy.resource.map(|r| r.into()).unwrap_or_default(),
            policy.policy.map(|p| p.into()).unwrap_or_default(),
        )
    }
}

/// Converts Protocol Buffer References to internal node-resources tuple.
impl From<proto::primitives::References> for (String, HashSet<Resource>) {
    fn from(references: proto::primitives::References) -> Self {
        (references.node, references.resources.into_iter().map(|r| r.into()).collect())
    }
}

/// Converts Protocol Buffer UpdateProvenance request to internal M2M request.
impl From<proto::messages::UpdateProvenance> for M2mRequest {
    fn from(req: proto::messages::UpdateProvenance) -> Self {
        M2mRequest::UpdateProvenance {
            source_prov: req.source_prov.into_iter().map(|s| s.into()).collect(),
            destination: req.destination.map(|d| d.into()).unwrap_or_default(),
        }
    }
}

/// Converts internal Policy to Protocol Buffer DestinationCompliance response.
impl From<Policy> for proto::messages::DestinationCompliance {
    fn from(policy: Policy) -> Self {
        proto::messages::DestinationCompliance { policy: Some(policy.into()) }
    }
}

/// Converts internal resource-policy map to Protocol Buffer SourceCompliance response.
impl From<HashMap<Resource, Policy>> for proto::messages::SourceCompliance {
    fn from(policies: HashMap<Resource, Policy>) -> Self {
        proto::messages::SourceCompliance {
            policies: policies
                .into_iter()
                .map(|(resource, policy)| proto::primitives::MappedPolicy {
                    resource: Some(resource.into()),
                    policy: Some(policy.into()),
                })
                .collect(),
        }
    }
}

/// Converts internal node-resources tuple to Protocol Buffer References.
impl From<(String, HashSet<Resource>)> for proto::primitives::References {
    fn from((node, resources): (String, HashSet<Resource>)) -> Self {
        proto::primitives::References {
            node,
            resources: resources.into_iter().map(|r| r.into()).collect(),
        }
    }
}

/// Converts internal resource-policy tuple to Protocol Buffer MappedPolicy.
impl From<(Resource, Policy)> for proto::primitives::MappedPolicy {
    fn from((resource, policy): (Resource, Policy)) -> Self {
        proto::primitives::MappedPolicy {
            resource: Some(resource.into()),
            policy: Some(policy.into()),
        }
    }
}

/// Converts Protocol Buffer Resource to internal Resource type.
impl From<proto::primitives::Resource> for Resource {
    fn from(proto_resource: proto::primitives::Resource) -> Self {
        match proto_resource.resource {
            Some(proto::primitives::resource::Resource::Fd(fd)) => Resource::Fd(fd.into()),
            Some(proto::primitives::resource::Resource::Process(process)) => {
                Resource::Process(process.into())
            }
            None => Resource::None,
        }
    }
}

/// Converts internal Resource to Protocol Buffer Resource type.
impl From<Resource> for proto::primitives::Resource {
    fn from(resource: Resource) -> Self {
        match resource {
            Resource::Fd(fd) => proto::primitives::Resource {
                resource: Some(proto::primitives::resource::Resource::Fd(fd.into())),
            },
            Resource::Process(process) => proto::primitives::Resource {
                resource: Some(proto::primitives::resource::Resource::Process(process.into())),
            },
            Resource::None => proto::primitives::Resource { resource: None },
        }
    }
}

impl From<proto::primitives::Fd> for Fd {
    fn from(proto_fd: proto::primitives::Fd) -> Self {
        match proto_fd.fd {
            Some(proto::primitives::fd::Fd::File(file)) => Fd::File(file.into()),
            Some(proto::primitives::fd::Fd::Stream(stream)) => Fd::Stream(stream.into()),
            None => Fd::File(File { path: String::new() }), // Default to empty file
        }
    }
}

impl From<Fd> for proto::primitives::Fd {
    fn from(fd: Fd) -> Self {
        match fd {
            Fd::File(file) => {
                proto::primitives::Fd { fd: Some(proto::primitives::fd::Fd::File(file.into())) }
            }
            Fd::Stream(stream) => {
                proto::primitives::Fd { fd: Some(proto::primitives::fd::Fd::Stream(stream.into())) }
            }
        }
    }
}

impl From<proto::primitives::File> for File {
    fn from(proto_file: proto::primitives::File) -> Self {
        File { path: proto_file.path }
    }
}

impl From<File> for proto::primitives::File {
    fn from(file: File) -> Self {
        proto::primitives::File { path: file.path }
    }
}

impl From<proto::primitives::Stream> for Stream {
    fn from(proto_stream: proto::primitives::Stream) -> Self {
        Stream { local_socket: proto_stream.local_socket, peer_socket: proto_stream.peer_socket }
    }
}

impl From<Stream> for proto::primitives::Stream {
    fn from(stream: Stream) -> Self {
        proto::primitives::Stream {
            local_socket: stream.local_socket,
            peer_socket: stream.peer_socket,
        }
    }
}

impl From<proto::primitives::Process> for Process {
    fn from(proto_process: proto::primitives::Process) -> Self {
        Process {
            pid: proto_process.pid,
            starttime: proto_process.starttime,
            exe_path: proto_process.exe_path,
        }
    }
}

impl From<Process> for proto::primitives::Process {
    fn from(process: Process) -> Self {
        proto::primitives::Process {
            pid: process.pid,
            starttime: process.starttime,
            exe_path: process.exe_path,
        }
    }
}

impl From<Policy> for proto::primitives::Policy {
    fn from(policy: Policy) -> Self {
        proto::primitives::Policy {
            confidentiality: match policy.is_confidential() {
                false => proto::primitives::Confidentiality::Public as i32,
                true => proto::primitives::Confidentiality::Secret as i32,
            },
            integrity: policy.get_integrity(),
            deleted: policy.is_deleted(),
            consent: policy.get_consent(),
        }
    }
}

impl From<proto::primitives::Policy> for Policy {
    fn from(proto_policy: proto::primitives::Policy) -> Self {
        Policy::new(
            match proto_policy.confidentiality {
                x if x == proto::primitives::Confidentiality::Secret as i32 => {
                    ConfidentialityPolicy::Secret
                }
                _ => ConfidentialityPolicy::Public,
            },
            proto_policy.integrity,
            proto_policy.deleted.into(),
            proto_policy.consent,
        )
    }
}
