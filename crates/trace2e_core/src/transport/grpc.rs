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
//! - **P2mHandler**: Server implementation that routes incoming gRPC requests for P2M operations
//! - **M2mHandler**: Server implementation that routes incoming gRPC requests for M2M operations
//! - **O2mHandler**: Server implementation that routes incoming gRPC requests for O2M operations
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
//! ### Operator-to-Middleware (O2M)
//! - Policy management
//! - Confidentiality management
//! - Integrity management
//! - Deletion management
//! - Consent management
//! - Provenance information retrieval
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
use futures::future::try_join_all;
use tokio_stream::{StreamExt, wrappers::BroadcastStream};
use tonic::{Request, Response, Status, transport::Channel};
use tower::Service;
use tracing::info;

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
        api::types::{M2mRequest, M2mResponse, O2mRequest, O2mResponse, P2mRequest, P2mResponse},
        error::TraceabilityError,
        infrastructure::naming::{
            DisplayableResource, Fd, File, LocalizedResource, Process, Resource, Stream,
        },
        services::{
            compliance::{ConfidentialityPolicy, Policy},
            consent::Destination,
        },
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
    connected_remotes: Arc<DashMap<String, proto::m2m_client::M2mClient<Channel>>>,
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
    ) -> Result<proto::m2m_client::M2mClient<Channel>, TraceabilityError> {
        match proto::m2m_client::M2mClient::connect(format!(
            "http://{remote_ip}:{DEFAULT_GRPC_PORT}"
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
    async fn get_client(&self, remote_ip: String) -> Option<proto::m2m_client::M2mClient<Channel>> {
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
    ) -> Result<proto::m2m_client::M2mClient<Channel>, TraceabilityError> {
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
            match request.clone() {
                M2mRequest::GetDestinationPolicy(destination) => {
                    info!(
                        destination = %destination,
                        "[gRPC-client] GetDestinationPolicy"
                    );
                    let remote_ip = eval_remote_ip(request)?;
                    let mut client = this.get_client_or_connect(remote_ip.clone()).await?;

                    // Create the protobuf request
                    let proto_req = proto::messages::GetDestinationPolicy {
                        destination: Some(destination.into()),
                    };

                    // Make the gRPC call
                    let response = client
                        .m2m_destination_policy(Request::new(proto_req))
                        .await
                        .map_err(|_| TraceabilityError::TransportFailedToContactRemote(remote_ip))?
                        .into_inner();
                    Ok(M2mResponse::DestinationPolicy(
                        response.policy.map(|policy| policy.into()).unwrap_or_default(),
                    ))
                }
                M2mRequest::CheckSourceCompliance { sources, destination } => {
                    info!(
                        sources = %DisplayableResource::from(&sources),
                        destination = %destination.0,
                        destination_policy = ?destination.1,
                        "[gRPC-client] CheckSourceCompliance"
                    );
                    // Create sources partition by node_id
                    let sources_partition = sources.iter().fold(
                        HashMap::new(),
                        |mut partitions: HashMap<&String, HashSet<&LocalizedResource>>, lr| {
                            partitions.entry(lr.node_id()).or_default().insert(lr);
                            partitions
                        },
                    );

                    let futures = sources_partition
                        .into_iter()
                        .map(|(node_id, sources)| {
                            let this_clone = this.clone();
                            let dest_resource = destination.0.clone();
                            let dest_policy = destination.1.clone();
                            async move {
                                let mut client =
                                    this_clone.get_client_or_connect(node_id.to_string()).await?;
                                client
                                    .m2m_check_source_compliance(Request::new(
                                        proto::messages::CheckSourceCompliance {
                                            sources: sources
                                                .iter()
                                                .map(|r| (**r).clone().into())
                                                .collect(),
                                            destination: Some((dest_resource).into()),
                                            destination_policy: Some((dest_policy).into()),
                                        },
                                    ))
                                    .await
                                    .map_err(|_| {
                                        TraceabilityError::TransportFailedToContactRemote(
                                            node_id.to_string(),
                                        )
                                    })
                            }
                        })
                        .collect::<Vec<_>>();

                    // Collect all results and return error if any failed
                    try_join_all(futures).await?;
                    Ok(M2mResponse::Ack)
                }
                M2mRequest::UpdateProvenance { source_prov, destination } => {
                    info!(
                        source_prov = %DisplayableResource::from(&source_prov),
                        destination = %destination,
                        "[gRPC-client] UpdateProvenance"
                    );
                    let remote_ip = eval_remote_ip(request)?;
                    let mut client = this.get_client_or_connect(remote_ip.clone()).await?;

                    // Group LocalizedResources by node_id
                    let mut grouped: HashMap<String, Vec<proto::primitives::Resource>> =
                        HashMap::default();
                    for lr in source_prov {
                        grouped
                            .entry(lr.node_id().clone())
                            .or_default()
                            .push(lr.resource().clone().into());
                    }

                    // Convert to Vec<References>
                    let source_prov_proto: Vec<proto::primitives::References> = grouped
                        .into_iter()
                        .map(|(node, resources)| proto::primitives::References { node, resources })
                        .collect();

                    // Create the protobuf request
                    let proto_req = proto::messages::UpdateProvenance {
                        source_prov: source_prov_proto,
                        destination: Some(destination.into()),
                    };

                    // Make the gRPC call
                    client.m2m_update_provenance(Request::new(proto_req)).await.map_err(|_| {
                        TraceabilityError::TransportFailedToContactRemote(remote_ip)
                    })?;

                    Ok(M2mResponse::Ack)
                }
                M2mRequest::BroadcastDeletion(resource) => {
                    info!(
                        resource = %resource,
                        "[gRPC-client] BroadcastDeletion"
                    );
                    let remote_ip = eval_remote_ip(request)?;
                    let mut client = this.get_client_or_connect(remote_ip.clone()).await?;

                    // Create the protobuf request
                    let proto_req = proto::messages::BroadcastDeletionRequest {
                        resource: Some(resource.into()),
                    };

                    // Make the gRPC call
                    client.m2m_broadcast_deletion(Request::new(proto_req)).await.map_err(|_| {
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
pub struct P2mHandler<P2mApi> {
    /// Process-to-middleware service handler.
    p2m: P2mApi,
}

impl<P2mApi> P2mHandler<P2mApi> {
    /// Creates a new router with the specified service handlers.
    ///
    /// # Arguments
    ///
    /// * `p2m` - Service for handling process-to-middleware requests
    pub fn new(p2m: P2mApi) -> Self {
        Self { p2m }
    }
}

/// Implementation of the trace2e gRPC service protocol.
///
/// This implementation provides the server-side handlers for all gRPC endpoints
/// defined in the trace2e protocol. It handles both P2M (process-to-middleware)
/// and M2M (machine-to-machine) operations by delegating to the appropriate
/// internal service handlers.
#[tonic::async_trait]
impl<P2mApi> proto::p2m_server::P2m for P2mHandler<P2mApi>
where
    P2mApi: Service<P2mRequest, Response = P2mResponse, Error = TraceabilityError>
        + Clone
        + Sync
        + Send
        + 'static,
    P2mApi::Future: Send,
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
}
pub struct M2mHandler<M2mApi> {
    /// Machine-to-machine service handler.
    m2m: M2mApi,
}

impl<M2mApi> M2mHandler<M2mApi> {
    /// Creates a new router with the specified service handlers.
    ///
    /// # Arguments
    ///
    /// * `m2m` - Service for handling machine-to-machine requests
    pub fn new(m2m: M2mApi) -> Self {
        Self { m2m }
    }
}

#[tonic::async_trait]
impl<M2mApi> proto::m2m_server::M2m for M2mHandler<M2mApi>
where
    M2mApi: Service<M2mRequest, Response = M2mResponse, Error = TraceabilityError>
        + Clone
        + Sync
        + Send
        + 'static,
    M2mApi::Future: Send,
{
    /// Handles destination policy requests from remote middleware.
    ///
    /// Returns the compliance policy for a destination resource to enable
    /// remote middleware instances to evaluate flow authorization.
    async fn m2m_destination_policy(
        &self,
        request: Request<proto::messages::GetDestinationPolicy>,
    ) -> Result<Response<proto::messages::DestinationPolicy>, Status> {
        info!("[gRPC-server] m2m_destination_policy");
        let req = request.into_inner();
        let mut m2m = self.m2m.clone();
        match m2m.call(req.into()).await? {
            M2mResponse::DestinationPolicy(policy) => Ok(Response::new(policy.into())),
            _ => Err(Status::internal("Internal traceability API error")),
        }
    }

    /// Handles source compliance checking requests from remote middleware.
    ///
    /// Checks compliance policies for a set of source resources to enable
    /// distributed flow evaluation across multiple middleware instances.
    async fn m2m_check_source_compliance(
        &self,
        request: Request<proto::messages::CheckSourceCompliance>,
    ) -> Result<Response<proto::messages::Ack>, Status> {
        info!("[gRPC-server] m2m_check_source_compliance");
        let req = request.into_inner();
        let mut m2m = self.m2m.clone();
        m2m.call(req.into()).await?;
        Ok(Response::new(proto::messages::Ack {}))
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
        info!("[gRPC-server] m2m_update_provenance");
        let req = request.into_inner();
        let mut m2m = self.m2m.clone();
        match m2m.call(req.into()).await? {
            M2mResponse::Ack => Ok(Response::new(proto::messages::Ack {})),
            _ => Err(Status::internal("Internal traceability API error")),
        }
    }

    /// Handles deletion broadcast requests from remote middleware.
    ///
    /// Broadcasts the deletion of a resource to all middleware instances.
    async fn m2m_broadcast_deletion(
        &self,
        request: Request<proto::messages::BroadcastDeletionRequest>,
    ) -> Result<Response<proto::messages::Ack>, Status> {
        info!("[gRPC-server] m2m_broadcast_deletion");
        let req = request.into_inner();
        let mut m2m = self.m2m.clone();
        match m2m.call(req.into()).await? {
            M2mResponse::Ack => Ok(Response::new(proto::messages::Ack {})),
            _ => Err(Status::internal("Internal traceability API error")),
        }
    }
}

/// gRPC server handler for operator-to-middleware operations.
///
/// `O2mHandler` implements the gRPC server-side logic for administrative
/// operations including policy management, provenance queries, and consent
/// management by operators and compliance officers.
///
/// ## Type Parameters
///
/// * `O2mApi` - Service handling operator-to-middleware requests
pub struct O2mHandler<O2mApi> {
    /// Operator-to-middleware service handler.
    o2m: O2mApi,
}

impl<O2mApi> O2mHandler<O2mApi> {
    /// Creates a new O2M handler with the specified service handler.
    ///
    /// # Arguments
    ///
    /// * `o2m` - Service for handling operator-to-middleware requests
    pub fn new(o2m: O2mApi) -> Self {
        Self { o2m }
    }
}

/// Implementation of the O2M gRPC service protocol.
///
/// This implementation provides the server-side handlers for all operator-facing
/// endpoints including policy configuration and provenance analysis.
#[tonic::async_trait]
impl<O2mApi> proto::o2m_server::O2m for O2mHandler<O2mApi>
where
    O2mApi: Service<O2mRequest, Response = O2mResponse, Error = TraceabilityError>
        + Clone
        + Sync
        + Send
        + 'static,
    O2mApi::Future: Send,
{
    /// Handles policy retrieval requests from operators.
    ///
    /// Returns the current compliance policies for the specified set of resources.
    async fn o2m_get_policies(
        &self,
        request: Request<proto::messages::GetPoliciesRequest>,
    ) -> Result<Response<proto::messages::GetPoliciesResponse>, Status> {
        let req = request.into_inner();
        let mut o2m = self.o2m.clone();
        match o2m.call(req.into()).await? {
            O2mResponse::Policies(policies) => Ok(Response::new(policies.into())),
            _ => Err(Status::internal("Internal traceability API error")),
        }
    }

    /// Handles policy update requests from operators.
    ///
    /// Sets a complete compliance policy for a specific resource.
    async fn o2m_set_policy(
        &self,
        request: Request<proto::messages::SetPolicyRequest>,
    ) -> Result<Response<proto::messages::Ack>, Status> {
        let req = request.into_inner();
        let mut o2m = self.o2m.clone();
        match o2m.call(req.into()).await? {
            O2mResponse::Ack => Ok(Response::new(proto::messages::Ack {})),
            _ => Err(Status::internal("Internal traceability API error")),
        }
    }

    /// Handles confidentiality setting requests from operators.
    ///
    /// Updates the confidentiality policy for a specific resource.
    async fn o2m_set_confidentiality(
        &self,
        request: Request<proto::messages::SetConfidentialityRequest>,
    ) -> Result<Response<proto::messages::Ack>, Status> {
        let req = request.into_inner();
        let mut o2m = self.o2m.clone();
        match o2m.call(req.into()).await? {
            O2mResponse::Ack => Ok(Response::new(proto::messages::Ack {})),
            _ => Err(Status::internal("Internal traceability API error")),
        }
    }

    /// Handles integrity setting requests from operators.
    ///
    /// Updates the integrity level for a specific resource.
    async fn o2m_set_integrity(
        &self,
        request: Request<proto::messages::SetIntegrityRequest>,
    ) -> Result<Response<proto::messages::Ack>, Status> {
        let req = request.into_inner();
        let mut o2m = self.o2m.clone();
        match o2m.call(req.into()).await? {
            O2mResponse::Ack => Ok(Response::new(proto::messages::Ack {})),
            _ => Err(Status::internal("Internal traceability API error")),
        }
    }

    /// Handles deletion marking requests from operators.
    ///
    /// Marks a resource as deleted for compliance tracking purposes.
    async fn o2m_set_deleted(
        &self,
        request: Request<proto::messages::SetDeletedRequest>,
    ) -> Result<Response<proto::messages::Ack>, Status> {
        let req = request.into_inner();
        let mut o2m = self.o2m.clone();
        match o2m.call(req.into()).await? {
            O2mResponse::Ack => Ok(Response::new(proto::messages::Ack {})),
            _ => Err(Status::internal("Internal traceability API error")),
        }
    }

    type O2MEnforceConsentStream = Pin<
        Box<
            dyn tokio_stream::Stream<Item = Result<proto::messages::ConsentNotification, Status>>
                + Send,
        >,
    >;

    /// Handles consent enforcement requests from operators.
    ///
    /// Enables consent enforcement for a resource and returns a stream of
    /// consent request notifications that can be monitored by the operator.
    async fn o2m_enforce_consent(
        &self,
        request: Request<proto::messages::EnforceConsentRequest>,
    ) -> Result<Response<Self::O2MEnforceConsentStream>, Status> {
        let req = request.into_inner();
        let mut o2m = self.o2m.clone();
        match o2m.call(req.into()).await? {
            O2mResponse::Notifications(receiver) => {
                // Convert the broadcast receiver into a stream of consent notifications
                let stream = BroadcastStream::new(receiver).map(|result| {
                    match result {
                        Ok(destination) => {
                            // Format destination as human-readable consent request message
                            let consent_request =
                                format!("Consent request for destination: {:?}", destination);
                            Ok(proto::messages::ConsentNotification { consent_request })
                        }
                        Err(e) => {
                            Err(Status::internal(format!("Notification stream error: {}", e)))
                        }
                    }
                });
                Ok(Response::new(Box::pin(stream)))
            }
            _ => Err(Status::internal("Internal traceability API error")),
        }
    }

    /// Handles consent decision requests from operators.
    ///
    /// Sets the consent decision for a specific data flow operation.
    async fn o2m_set_consent_decision(
        &self,
        request: Request<proto::messages::SetConsentDecisionRequest>,
    ) -> Result<Response<proto::messages::Ack>, Status> {
        let req = request.into_inner();
        let mut o2m = self.o2m.clone();
        match o2m.call(req.into()).await? {
            O2mResponse::Ack => Ok(Response::new(proto::messages::Ack {})),
            _ => Err(Status::internal("Internal traceability API error")),
        }
    }

    /// Handles provenance query requests from operators.
    ///
    /// Returns the complete provenance lineage for a specific resource.
    async fn o2m_get_references(
        &self,
        request: Request<proto::messages::GetReferencesRequest>,
    ) -> Result<Response<proto::messages::GetReferencesResponse>, Status> {
        let req = request.into_inner();
        let mut o2m = self.o2m.clone();
        match o2m.call(req.into()).await? {
            O2mResponse::References(references) => {
                // Group LocalizedResources by node_id
                let mut grouped: HashMap<String, HashSet<Resource>> = HashMap::default();
                for lr in references {
                    grouped.entry(lr.node_id().clone()).or_default().insert(lr.resource().clone());
                }
                Ok(Response::new(grouped.into()))
            }
            _ => Err(Status::internal("Internal traceability API error")),
        }
    }
}

// ========== Protocol Buffer Type Conversions ==========
// Fundamental type conversions for serialization/deserialization

/// Converts Protocol Buffer Resource to internal Resource type.
impl From<proto::primitives::Resource> for Resource {
    fn from(req: proto::primitives::Resource) -> Self {
        match req.resource {
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
            None => Fd::File(File { path: String::new() }),
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

// ========== LocalizedResource and Destination Conversions ==========

/// Converts Protocol Buffer LocalizedResource to internal LocalizedResource type.
impl From<proto::primitives::LocalizedResource> for LocalizedResource {
    fn from(proto_lr: proto::primitives::LocalizedResource) -> Self {
        LocalizedResource::new(
            proto_lr.node_id,
            proto_lr.resource.map(|r| r.into()).unwrap_or_default(),
        )
    }
}

/// Converts internal LocalizedResource to Protocol Buffer LocalizedResource type.
impl From<LocalizedResource> for proto::primitives::LocalizedResource {
    fn from(lr: LocalizedResource) -> Self {
        proto::primitives::LocalizedResource {
            node_id: lr.node_id().clone(),
            resource: Some(lr.resource().clone().into()),
        }
    }
}

/// Converts Protocol Buffer Destination to internal Destination type.
impl From<proto::primitives::Destination> for Destination {
    fn from(proto_dest: proto::primitives::Destination) -> Self {
        match proto_dest.destination {
            Some(proto::primitives::destination::Destination::Resource(lr_with_parent)) => {
                if let Some(proto_lr) = lr_with_parent.resource {
                    let localized_resource: LocalizedResource = proto_lr.into();
                    let parent = lr_with_parent.parent.map(|p| Box::new((*p).into()));
                    // Preserve the LocalizedResource as a Resource with node_id in the parent
                    Destination::Resource {
                        resource: localized_resource.resource().clone(),
                        parent,
                    }
                } else {
                    Destination::Resource { resource: Resource::None, parent: None }
                }
            }
            Some(proto::primitives::destination::Destination::Node(node_id)) => {
                Destination::Node(node_id)
            }
            None => Destination::Node(String::new()),
        }
    }
}

/// Converts internal Destination to Protocol Buffer Destination type.
/// Preserves LocalizedResource context by reconstructing the hierarchical structure.
impl From<Destination> for proto::primitives::Destination {
    fn from(dest: Destination) -> Self {
        destination_to_proto_node_variant(dest)
    }
}

/// Helper function that performs the actual Destination -> proto conversion logic.
/// This can be reused by other modules that need the same conversion.
pub fn destination_to_proto_node_variant(dest: Destination) -> proto::primitives::Destination {
    match dest {
        Destination::Resource { resource, parent } => {
            // When converting back to proto, we reconstruct the LocalizedResource
            // If parent is None, we use empty string for node_id (local)
            // If parent is Some(Node(...)), we extract the node_id
            let (node_id, proto_parent) = match &parent {
                Some(p) => {
                    match &(**p) {
                        Destination::Node(node) => {
                            (node.clone(), parent.map(|p| Box::new((*p).clone().into())))
                        }
                        _ => {
                            // If parent is another Resource, preserve it as-is
                            (String::new(), parent.map(|p| Box::new((*p).clone().into())))
                        }
                    }
                }
                None => {
                    // No parent, local resource
                    (String::new(), None)
                }
            };

            proto::primitives::Destination {
                destination: Some(proto::primitives::destination::Destination::Resource(Box::new(
                    proto::primitives::LocalizedResourceWithParent {
                        resource: Some(LocalizedResource::new(node_id, resource).into()),
                        parent: proto_parent,
                    },
                ))),
            }
        }
        Destination::Node(node_id) => proto::primitives::Destination {
            destination: Some(proto::primitives::destination::Destination::Node(node_id)),
        },
    }
}

// ========== Policy Conversions ==========

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

// ========== Resource-Policy Mapping Conversions ==========

/// Converts Protocol Buffer MappedLocalizedPolicy to internal tuple.
impl From<proto::primitives::MappedLocalizedPolicy> for (LocalizedResource, Policy) {
    fn from(policy: proto::primitives::MappedLocalizedPolicy) -> Self {
        (
            policy.resource.map(|r| r.into()).unwrap_or_default(),
            policy.policy.map(|p| p.into()).unwrap_or_default(),
        )
    }
}

// ========== Provenance References Conversions ==========

/// Converts Protocol Buffer References to internal node-resources tuple.
impl From<proto::primitives::References> for (String, HashSet<Resource>) {
    fn from(references: proto::primitives::References) -> Self {
        (references.node, references.resources.into_iter().map(|r| r.into()).collect())
    }
}

/// Converts Protocol Buffer References to LocalizedResource set.
impl From<proto::primitives::References> for HashSet<LocalizedResource> {
    fn from(references: proto::primitives::References) -> Self {
        references
            .resources
            .into_iter()
            .map(|r| LocalizedResource::new(references.node.clone(), r.into()))
            .collect()
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

// ========== M2M Protocol Buffer Conversions ==========

/// Converts Protocol Buffer GetDestinationPolicy request to internal M2M request.
impl From<proto::messages::GetDestinationPolicy> for M2mRequest {
    fn from(req: proto::messages::GetDestinationPolicy) -> Self {
        M2mRequest::GetDestinationPolicy(req.destination.map(|d| d.into()).unwrap_or_default())
    }
}

/// Converts Protocol Buffer UpdateProvenance request to internal M2M request.
impl From<proto::messages::UpdateProvenance> for M2mRequest {
    fn from(req: proto::messages::UpdateProvenance) -> Self {
        let source_prov: HashSet<LocalizedResource> = req
            .source_prov
            .into_iter()
            .flat_map(|refs: proto::primitives::References| {
                let node_id = refs.node.clone();
                refs.resources
                    .into_iter()
                    .map(move |r| LocalizedResource::new(node_id.clone(), r.into()))
            })
            .collect();

        M2mRequest::UpdateProvenance {
            source_prov,
            destination: req.destination.map(|d| d.into()).unwrap_or_default(),
        }
    }
}

/// Converts Protocol Buffer BroadcastDeletionRequest to internal M2M request.
impl From<proto::messages::BroadcastDeletionRequest> for M2mRequest {
    fn from(req: proto::messages::BroadcastDeletionRequest) -> Self {
        M2mRequest::BroadcastDeletion(req.resource.map(|r| r.into()).unwrap_or_default())
    }
}

/// Converts Protocol Buffer CheckSourceCompliance request to internal M2M request.
impl From<proto::messages::CheckSourceCompliance> for M2mRequest {
    fn from(req: proto::messages::CheckSourceCompliance) -> Self {
        let sources: HashSet<LocalizedResource> =
            req.sources.into_iter().map(|s| s.into()).collect();
        let destination: LocalizedResource = req.destination.map(|d| d.into()).unwrap_or_default();
        let destination_policy: Policy =
            req.destination_policy.map(|p| p.into()).unwrap_or_default();
        M2mRequest::CheckSourceCompliance {
            sources,
            destination: (destination, destination_policy),
        }
    }
}

/// Converts internal M2M DestinationPolicy response to Protocol Buffer response.
impl From<Policy> for proto::messages::DestinationPolicy {
    fn from(policy: Policy) -> Self {
        proto::messages::DestinationPolicy { policy: Some(policy.into()) }
    }
}

// ========== O2M Protocol Buffer Conversions ==========

/// Converts Protocol Buffer GetPoliciesRequest to internal O2M request.
impl From<proto::messages::GetPoliciesRequest> for O2mRequest {
    fn from(req: proto::messages::GetPoliciesRequest) -> Self {
        O2mRequest::GetPolicies(req.resources.into_iter().map(|r| r.into()).collect())
    }
}

/// Converts Protocol Buffer SetPolicyRequest to internal O2M request.
impl From<proto::messages::SetPolicyRequest> for O2mRequest {
    fn from(req: proto::messages::SetPolicyRequest) -> Self {
        O2mRequest::SetPolicy {
            resource: req.resource.map(|r| r.into()).unwrap_or_default(),
            policy: req.policy.map(|p| p.into()).unwrap_or_default(),
        }
    }
}

/// Converts Protocol Buffer SetConfidentialityRequest to internal O2M request.
impl From<proto::messages::SetConfidentialityRequest> for O2mRequest {
    fn from(req: proto::messages::SetConfidentialityRequest) -> Self {
        O2mRequest::SetConfidentiality {
            resource: req.resource.map(|r| r.into()).unwrap_or_default(),
            confidentiality: match req.confidentiality {
                x if x == proto::primitives::Confidentiality::Secret as i32 => {
                    ConfidentialityPolicy::Secret
                }
                _ => ConfidentialityPolicy::Public,
            },
        }
    }
}

/// Converts Protocol Buffer SetIntegrityRequest to internal O2M request.
impl From<proto::messages::SetIntegrityRequest> for O2mRequest {
    fn from(req: proto::messages::SetIntegrityRequest) -> Self {
        O2mRequest::SetIntegrity {
            resource: req.resource.map(|r| r.into()).unwrap_or_default(),
            integrity: req.integrity,
        }
    }
}

/// Converts Protocol Buffer SetDeletedRequest to internal O2M request.
impl From<proto::messages::SetDeletedRequest> for O2mRequest {
    fn from(req: proto::messages::SetDeletedRequest) -> Self {
        O2mRequest::SetDeleted(req.resource.map(|r| r.into()).unwrap_or_default())
    }
}

/// Converts Protocol Buffer EnforceConsentRequest to internal O2M request.
impl From<proto::messages::EnforceConsentRequest> for O2mRequest {
    fn from(req: proto::messages::EnforceConsentRequest) -> Self {
        O2mRequest::EnforceConsent(req.resource.map(|r| r.into()).unwrap_or_default())
    }
}

/// Converts Protocol Buffer SetConsentDecisionRequest to internal O2M request.
impl From<proto::messages::SetConsentDecisionRequest> for O2mRequest {
    fn from(req: proto::messages::SetConsentDecisionRequest) -> Self {
        O2mRequest::SetConsentDecision {
            source: req.source.map(|r| r.into()).unwrap_or_default(),
            destination: req
                .destination
                .map(|d| d.into())
                .unwrap_or_else(|| Destination::Node(String::new())),
            decision: req.decision,
        }
    }
}

/// Converts Protocol Buffer GetReferencesRequest to internal O2M request.
impl From<proto::messages::GetReferencesRequest> for O2mRequest {
    fn from(req: proto::messages::GetReferencesRequest) -> Self {
        O2mRequest::GetReferences(req.resource.map(|r| r.into()).unwrap_or_default())
    }
}

// ========== O2M Response Conversions ==========

/// Converts internal resource-policy map to Protocol Buffer GetPoliciesResponse.
impl From<HashMap<Resource, Policy>> for proto::messages::GetPoliciesResponse {
    fn from(policies: HashMap<Resource, Policy>) -> Self {
        proto::messages::GetPoliciesResponse {
            policies: policies
                .into_iter()
                .map(|(resource, policy)| proto::primitives::MappedLocalizedPolicy {
                    resource: Some(LocalizedResource::new(String::new(), resource).into()),
                    policy: Some(policy.into()),
                })
                .collect(),
        }
    }
}

/// Converts internal LocalizedResource-policy map to Protocol Buffer GetPoliciesResponse.
impl From<HashMap<LocalizedResource, Policy>> for proto::messages::GetPoliciesResponse {
    fn from(policies: HashMap<LocalizedResource, Policy>) -> Self {
        proto::messages::GetPoliciesResponse {
            policies: policies
                .into_iter()
                .map(|(resource, policy)| proto::primitives::MappedLocalizedPolicy {
                    resource: Some(resource.into()),
                    policy: Some(policy.into()),
                })
                .collect(),
        }
    }
}

/// Converts internal provenance references to Protocol Buffer GetReferencesResponse.
impl From<HashMap<String, HashSet<Resource>>> for proto::messages::GetReferencesResponse {
    fn from(references: HashMap<String, HashSet<Resource>>) -> Self {
        proto::messages::GetReferencesResponse {
            references: references
                .into_iter()
                .map(|(node, resources)| (node, resources).into())
                .collect(),
        }
    }
}
