use thiserror::Error;
use std::net::SocketAddr;

use crate::identifier::Identifier;

/// Traceability layer error type.
#[derive(Error, Debug)]
pub enum TraceabilityError {
    /// Flow declaration failed because the given containers are not registered.
    #[error("Traceability error: {0:?} and {1:?} are not registered.")]
    MissingRegistrations(Identifier, Identifier),
    /// Flow declaration failed because the given container is not registered.
    #[error("Traceability error: {0:?} is not registered.")]
    MissingRegistration(Identifier),
    /// Flow declaration failed because the type of the given containers is
    /// not valid.
    #[error("Traceability error: {0:?}<->{1:?} Flow is invalid.")]
    InvalidFlow(Identifier, Identifier),
    /// Flow declaration failed because it is not compliant.
    #[error("Traceability error: {0:?}<->{1:?} Flow is forbidden.")]
    ForbiddenFlow(Identifier, Identifier),
    /// Flow recording failed due to missing declaration/grant.
    #[error("Traceability error: unable to record Flow {0}.")]
    RecordingFailure(u64),
    /// Incoming Remote Update of the provenance failed.
    #[error("Traceability error: {0:?} is not registered, so traceability can not be enforced.")]
    MissingRegistrationStream(Identifier),
    /// Missing stream registration on remote (triggered by MissingRegistrationStream on remote)
    #[error("Traceability error: stream://tcp;{0};{1} is not registered on remote, so traceability can not be enforced.")]
    MissingRegistrationRemote(SocketAddr, SocketAddr),
    /// Communication failed with the remote middleware
    #[error("Traceability error: Communication failed with the remote middleware ({}).", .0)]
    NonCompliantRemote(SocketAddr),
    /// Channel error
    #[error("Traceability error: Internal channel error.")]
    ChannelError,
    /// Invalid response
    #[error("Traceability error: Internal invalid response.")]
    InvalidResponse,
}