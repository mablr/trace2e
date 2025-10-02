//! Operator-to-Middleware (O2M) API service implementation.
//!
//! This module provides the service implementation for handling administrative requests
//! from external operators, compliance officers, and organizations. The O2M API enables
//! governance operations including policy management, compliance configuration, and
//! provenance analysis across the traceability system.
//!
//! ## Service Architecture
//!
//! The `O2mApiService` coordinates between two core services:
//! - **Provenance Service**: For querying resource lineage and ancestry data
//! - **Compliance Service**: For policy management and configuration updates
//!
//! ## Supported Operations
//!
//! **Policy Management**: Set and retrieve compliance policies for resources including
//! confidentiality, integrity, consent, and deletion status.
//!
//! **Provenance Analysis**: Query complete resource lineage to understand data flows
//! and dependencies for audit and compliance purposes.
//!
//! ## Administrative Privileges
//!
//! O2M operations typically require elevated privileges and are intended for use by
//! authorized personnel responsible for data governance and regulatory compliance.

use std::{future::Future, pin::Pin, task::Poll};

use tower::Service;
#[cfg(feature = "trace2e_tracing")]
use tracing::info;

use crate::traceability::{
    api::types::{
        ComplianceRequest, ComplianceResponse, O2mRequest, O2mResponse, ProvenanceRequest,
        ProvenanceResponse,
    },
    error::TraceabilityError,
    infrastructure::naming::NodeId,
};

/// O2M (Operator-to-Middleware) API Service
///
/// This service handles traceability requests from external operators and organizations,
/// providing policy management capabilities and resource reference queries.
/// It coordinates between provenance and compliance services to serve external requests.
#[derive(Debug, Clone)]
pub struct O2mApiService<P, C> {
    /// Service for tracking resources provenance
    provenance: P,
    /// Service for policy management and compliance checking
    compliance: C,
}

impl<P, C> O2mApiService<P, C> {
    /// Creates a new O2M API service with the provided provenance and compliance services
    pub fn new(provenance: P, compliance: C) -> Self {
        Self { provenance, compliance }
    }
}

impl<P, C> Service<O2mRequest> for O2mApiService<P, C>
where
    P: Service<ProvenanceRequest, Response = ProvenanceResponse, Error = TraceabilityError>
        + Clone
        + Send
        + NodeId
        + 'static,
    P::Future: Send,
    C: Service<ComplianceRequest, Response = ComplianceResponse, Error = TraceabilityError>
        + Clone
        + Send
        + 'static,
    C::Future: Send,
{
    type Response = O2mResponse;
    type Error = TraceabilityError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut std::task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: O2mRequest) -> Self::Future {
        let mut provenance = self.provenance.clone();
        let mut compliance = self.compliance.clone();
        Box::pin(async move {
            match request {
                O2mRequest::GetPolicies(resources) => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!("[o2m-{}] GetPolicies: resources: {:?}", provenance.node_id(), resources);
                    match compliance.call(ComplianceRequest::GetPolicies(resources)).await? {
                        ComplianceResponse::Policies(policies) => {
                            Ok(O2mResponse::Policies(policies))
                        }
                        _ => Err(TraceabilityError::InternalTrace2eError),
                    }
                }
                O2mRequest::SetPolicy { resource, policy } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[o2m-{}] SetPolicy: resource: {:?}, policy: {:?}",
                        provenance.node_id(),
                        resource,
                        policy
                    );
                    match compliance.call(ComplianceRequest::SetPolicy { resource, policy }).await?
                    {
                        ComplianceResponse::PolicyUpdated => Ok(O2mResponse::Ack),
                        _ => Err(TraceabilityError::InternalTrace2eError),
                    }
                }
                O2mRequest::SetConfidentiality { resource, confidentiality } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[o2m-{}] SetConfidentiality: resource: {:?}, confidentiality: {:?}",
                        provenance.node_id(),
                        resource,
                        confidentiality
                    );
                    match compliance
                        .call(ComplianceRequest::SetConfidentiality { resource, confidentiality })
                        .await?
                    {
                        ComplianceResponse::PolicyUpdated => Ok(O2mResponse::Ack),
                        _ => Err(TraceabilityError::InternalTrace2eError),
                    }
                }
                O2mRequest::SetIntegrity { resource, integrity } => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!(
                        "[o2m-{}] SetIntegrity: resource: {:?}, integrity: {:?}",
                        provenance.node_id(),
                        resource,
                        integrity
                    );
                    match compliance
                        .call(ComplianceRequest::SetIntegrity { resource, integrity })
                        .await?
                    {
                        ComplianceResponse::PolicyUpdated => Ok(O2mResponse::Ack),
                        _ => Err(TraceabilityError::InternalTrace2eError),
                    }
                }
                O2mRequest::SetDeleted(resource) => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!("[o2m-{}] SetDeleted: resource: {:?}", provenance.node_id(), resource,);
                    match compliance.call(ComplianceRequest::SetDeleted(resource)).await? {
                        ComplianceResponse::PolicyUpdated => Ok(O2mResponse::Ack),
                        _ => Err(TraceabilityError::InternalTrace2eError),
                    }
                }
                O2mRequest::SetConsent(resource) => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!("[o2m-{}] SetConsent: resource: {:?}", provenance.node_id(), resource);
                    match compliance
                        .call(ComplianceRequest::SetConsent { resource, consent: true })
                        .await?
                    {
                        ComplianceResponse::PolicyUpdated => Ok(O2mResponse::Ack),
                        _ => Err(TraceabilityError::InternalTrace2eError),
                    }
                }
                O2mRequest::GetReferences(resource) => {
                    #[cfg(feature = "trace2e_tracing")]
                    info!("[o2m-{}] GetReferences: resource: {:?}", provenance.node_id(), resource);
                    match provenance.call(ProvenanceRequest::GetReferences(resource)).await? {
                        ProvenanceResponse::Provenance(references) => {
                            Ok(O2mResponse::References(references))
                        }
                        _ => Err(TraceabilityError::InternalTrace2eError),
                    }
                }
            }
        })
    }
}
