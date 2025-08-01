use std::net::SocketAddr;

use crate::traceability::{
    api::M2mRequest,
    error::TraceabilityError,
    naming::{Fd, Resource},
};

pub mod grpc;
pub mod loopback;
pub mod nop;

fn eval_remote_ip(req: M2mRequest) -> Result<String, TraceabilityError> {
    if let Some(peer_socket) = match req {
        M2mRequest::GetConsistentCompliance { destination, .. }
        | M2mRequest::ProvenanceUpdate { destination, .. } => match destination {
            Resource::Fd(Fd::Stream(stream)) => Some(stream.peer_socket),
            _ => None,
        },
        M2mRequest::GetLooseCompliance { authority_ip, .. } => Some(authority_ip),
    } {
        match peer_socket.parse::<SocketAddr>() {
            Ok(addr) => Ok(addr.ip().to_string()),
            Err(_) => Err(TraceabilityError::TransportFailedToEvaluateRemote),
        }
    } else {
        Err(TraceabilityError::TransportFailedToEvaluateRemote)
    }
}
