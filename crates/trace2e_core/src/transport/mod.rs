use std::net::SocketAddr;

use crate::traceability::{
    api::M2mRequest,
    error::TraceabilityError,
    naming::{Fd, Resource},
};

pub mod grpc;
pub mod loopback;
pub mod nop;

pub fn eval_remote_ip(req: M2mRequest) -> Result<String, TraceabilityError> {
    match req {
        M2mRequest::GetDestinationCompliance { destination, .. }
        | M2mRequest::UpdateProvenance { destination, .. }
        | M2mRequest::UpdatePolicies { destination, .. } => match destination {
            // Local socket is the remote socket of the remote stream
            Resource::Fd(Fd::Stream(stream)) => match stream.local_socket.parse::<SocketAddr>() {
                Ok(addr) => Ok(addr.ip().to_string()),
                Err(_) => Err(TraceabilityError::TransportFailedToEvaluateRemote),
            },
            _ => Err(TraceabilityError::TransportFailedToEvaluateRemote),
        },
        M2mRequest::GetSourceCompliance { authority_ip, .. } => Ok(authority_ip),
    }
}
