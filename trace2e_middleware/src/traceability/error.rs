use thiserror::Error;

#[derive(Debug, Error)]
pub enum P2mError {
    #[error("P2M error occurred")]
    DefaultError,

    #[error("Invalid request")]
    InvalidRequest,
}

#[derive(Debug, Error)]
pub enum ResourceError {
    #[error("Resource error occurred")]
    DefaultError,
}

#[derive(Debug, Error)]
pub enum ReservationError {
    #[error("Reservation failed, failed to lock the reservation state")]
    ReservationLockError,

    #[error("Reservation failed, waiting queue failure")]
    ReservationWaitingQueueError,

    #[error("Reservation failed, already reserved shared")]
    AlreadyReservedShared,

    #[error("Reservation failed, already reserved exclusive")]
    AlreadyReservedExclusive,
}
