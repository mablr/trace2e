use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReservationError {
    #[error("Reservation failure, waiting queue failure")]
    ReservationWaitingQueueError,

    #[error("Reservation failure, already reserved in shared mode")]
    AlreadyReservedShared,

    #[error("Reservation failure, already reserved in exclusive mode")]
    AlreadyReservedExclusive,

    #[error("Reservation failure, unauthorized release")]
    UnauthorizedRelease,
}
