use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum TraceabilityError {
    #[error("Traceability error, undeclared resource (pid: {0}, fd: {1})")]
    UndeclaredResource(i32, i32),

    #[error("Traceability error, process not found (pid: {0})")]
    InvalidProcess(i32),

    #[error("Traceability error, invalid stream (local_socket: {0}, peer_socket: {1})")]
    InvalidStream(String, String),

    #[error("Traceability error, flow not found (id: {0})")]
    NotFoundFlow(usize),
}

#[derive(Debug, Error, PartialEq)]
pub enum ReservationError {
    #[error("Reservation failure, already reserved in shared mode")]
    AlreadyReservedShared,

    #[error("Reservation failure, already reserved in exclusive mode")]
    AlreadyReservedExclusive,

    #[error("Reservation failure, unauthorized release")]
    UnauthorizedRelease,
}
