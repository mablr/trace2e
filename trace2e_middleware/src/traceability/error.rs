use thiserror::Error;

use crate::traceability::naming::Identifier;

#[derive(Debug, Error, PartialEq)]
pub enum TraceabilityError {
    #[error("Traceability error, internal trace2e API error")]
    InternalTrace2eError,

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
    UnavailableDestination(Identifier),

    #[error("Traceability error, source unavailable")]
    UnavailableSource(Identifier),

    #[error("Traceability error, source and destination unavailable")]
    UnavailableSourceAndDestination(Identifier, Identifier),

    #[error("Traceability error, reached max retries waiting queue")]
    ReachedMaxRetriesWaitingQueue,
}
