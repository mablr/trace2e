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
