use std::net::SocketAddr;

use crate::identifier::Identifier;

/// Traceability layer error type.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum TraceabilityError {
    /// Flow declaration failed because one of the given containers is
    /// not registered so far in the middleware.
    MissingRegistration(Option<Identifier>, Option<Identifier>),
    /// Flow declaration failed because the type of the given containers is
    /// not valid.
    InvalidFlow(Identifier, Identifier),
    /// Flow declaration failed because it is not compliant.
    ForbiddenFlow(Identifier, Identifier),
    /// Flow recording failed due to missing declaration/grant.
    RecordingFailure(u64),
    /// Incoming Remote Update of the provenance failed.
    MissingRegistrationStream(Identifier),
    /// Missing stream registration on remote (triggered by MissingRegistrationStream on remote)
    MissingRegistrationRemote(SocketAddr, SocketAddr),
    /// Communication failed with the remote middleware
    NonCompliantRemote(SocketAddr),
}

impl std::fmt::Display for TraceabilityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TraceabilityError::MissingRegistration(id1, id2) => {
                if id1.is_some() && id2.is_none() {
                    write!(
                        f,
                        "Traceability error: {:?} is not registered.",
                        id1.clone().unwrap()
                    )
                } else if id1.is_none() && id2.is_some() {
                    write!(
                        f,
                        "Traceability error: {:?} is not registered.",
                        id2.clone().unwrap()
                    )
                } else if id1.is_none() && id2.is_none() {
                    write!(
                        f,
                        "Traceability error: invalid MissingRegistration parameters."
                    )
                } else {
                    write!(
                        f,
                        "Traceability error: ({:?} || {:?}) are not registered.",
                        id1.clone().unwrap(),
                        id2.clone().unwrap()
                    )
                }
            }
            TraceabilityError::InvalidFlow(id1, id2) => {
                write!(
                    f,
                    "Traceability error: {:?}<->{:?} Flow is invalid.",
                    id1, id2
                )
            }
            TraceabilityError::ForbiddenFlow(id1, id2) => {
                write!(
                    f,
                    "Traceability error: {:?}<->{:?} Flow is forbidden.",
                    id1, id2
                )
            }
            TraceabilityError::RecordingFailure(grant_id) => {
                write!(f, "Traceability error: unable to record Flow {}.", grant_id)
            }
            TraceabilityError::MissingRegistrationStream(id) => {
                write!(f, "Traceability error: {:?} is not registered, so traceability can not be enforced.", id)
            }
            TraceabilityError::MissingRegistrationRemote(local_socket, peer_socket) => {
                write!(f, "Traceability error: stream://tcp;{};{} is not registered on remote, so traceability can not be enforced.", local_socket, peer_socket)
            }
            TraceabilityError::NonCompliantRemote(s) => {
                write!(
                    f,
                    "Traceability error: Communication failed with the remote middleware ({}).",
                    s.ip()
                )
            }
        }
    }
}

impl std::error::Error for TraceabilityError {}
