use std::net::SocketAddr;

use crate::traceability::{
    api::M2mRequest,
    naming::{Fd, Resource},
};

pub mod grpc;
pub mod loopback;
pub mod nop;

fn eval_remote_ip(req: M2mRequest) -> Option<String> {
    if let Some(peer_socket) = match req {
        M2mRequest::ComplianceRetrieval { destination, .. }
        | M2mRequest::ProvenanceUpdate { destination, .. } => match destination.resource {
            Resource::Fd(Fd::Stream(stream)) => Some(stream.peer_socket),
            _ => None,
        },
    } {
        match peer_socket.parse::<SocketAddr>() {
            Ok(addr) => Some(addr.ip().to_string()),
            Err(_) => None,
        }
    } else {
        None
    }
}
