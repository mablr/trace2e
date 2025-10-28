//! Traceability error types and handling.
//!
//! This module defines comprehensive error types for the trace2e traceability system,
//! covering validation failures, resource management issues, policy violations, and
//! distributed system communication errors.
//!
//! ## Error Categories
//!
//! **Validation Errors**: Issues with process validation, resource enrollment, and request formatting
//!
//! **Resource Management**: Problems with resource discovery, availability, and lifecycle management
//!
//! **Policy and Compliance**: Authorization failures, policy violations, and compliance issues
//!
//! **System and Infrastructure**: Time synchronization, flow management, and retry limit errors
//!
//! **Network and Transport**: Communication failures between distributed middleware instances
//!
//! All errors implement the standard `Error` trait and provide detailed context for debugging
//! and operational monitoring.

use thiserror::Error;

use crate::traceability::infrastructure::naming::Resource;

#[derive(Debug, Error, PartialEq)]
pub enum TraceabilityError {
    #[error("Traceability error, internal trace2e API error")]
    InternalTrace2eError,

    #[error("Traceability error, invalid request")]
    InvalidRequest,

    #[error("Traceability error, undeclared resource (pid: {0}, fd: {1})")]
    UndeclaredResource(i32, i32),

    #[error("Traceability error, process not found (pid: {0})")]
    InvalidProcess(i32),

    #[error("Traceability error, invalid stream (local_socket: {0}, peer_socket: {1})")]
    InvalidStream(String, String),

    #[error("Traceability error, failed to instantiate flow due to system time error")]
    SystemTimeError,

    #[error("Traceability error, flow not found (id: {0})")]
    NotFoundFlow(u128),

    #[error("Traceability error, destination unavailable")]
    UnavailableDestination(Resource),

    #[error("Traceability error, source unavailable")]
    UnavailableSource(Resource),

    #[error("Traceability error, source and destination unavailable")]
    UnavailableSourceAndDestination(Resource, Resource),

    #[error("Traceability error, reached max retries waiting queue")]
    ReachedMaxRetriesWaitingQueue,

    #[error("Traceability error, direct policy violation")]
    DirectPolicyViolation,

    #[error("Traceability error, destination policy not found")]
    DestinationPolicyNotFound,

    #[error("Traceability error, invalid destination, bad request routing")]
    InvalidDestination,

    #[error("Traceability error, failed to contact remote middleware ({0})")]
    TransportFailedToContactRemote(String),

    #[error("Traceability error, transport layer failed to evaluate remote IP")]
    TransportFailedToEvaluateRemote,

    #[error("Traceability error, consent request timeout")]
    ConsentRequestTimeout,
}
