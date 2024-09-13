//! Global identification mechanism for data containers.

use std::net::SocketAddr;

use std::sync::OnceLock;

use crate::m2m_service::m2m::Id;

pub static MIDDLEWARE_ID: OnceLock<String> = OnceLock::new();

/// Global resource identification object.
///
/// Structure associating [`MIDDLEWARE_ID`] with a local resource
/// identification object to allow decentralized identification.
#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub struct Identifier {
    node: String,
    resource: Resource,
}

impl Identifier {
    pub fn new_file(path: String) -> Self {
        Self {
            node: MIDDLEWARE_ID
                .get_or_init(|| "localhost".to_string())
                .clone(),
            resource: Resource::File(path),
        }
    }

    pub fn new_stream(local_socket: SocketAddr, peer_socket: SocketAddr) -> Self {
        Self {
            node: MIDDLEWARE_ID
                .get_or_init(|| "localhost".to_string())
                .clone(),
            resource: Resource::Stream(local_socket, peer_socket),
        }
    }

    pub fn new_process(pid: u32, starttime: u64) -> Self {
        Self {
            node: MIDDLEWARE_ID
                .get_or_init(|| "localhost".to_string())
                .clone(),
            resource: Resource::Process(pid, starttime),
        }
    }

    pub fn is_file(&self) -> Option<&String> {
        match &self.resource {
            Resource::File(path) => Some(&path),
            _ => None,
        }
    }

    pub fn is_process(&self) -> Option<u32> {
        match self.resource {
            Resource::Process(pid, _) => Some(pid),
            _ => None,
        }
    }

    pub fn is_stream(&self) -> Option<(SocketAddr, SocketAddr)> {
        match self.resource {
            Resource::Stream(local_socket, peer_socket) => Some((local_socket, peer_socket)),
            _ => None,
        }
    }
}

impl From<Id> for Identifier {
    fn from(id: Id) -> Self {
        Identifier {
            node: id.node,
            // Assuming the resource field is always for a file.
            resource: Resource::File(id.resource),
        }
    }
}

impl From<Identifier> for Id {
    fn from(identifier_internal: Identifier) -> Self {
        let resource = match identifier_internal.resource {
            Resource::File(path) => path,
            _ => panic!("Only File variant is supported for Id conversion"),
        };
        Id {
            node: identifier_internal.node,
            resource,
        }
    }
}

/// Local resource identification object.
///
/// Enum that is instantiated when a resource is enrolled. It allows any type of
/// resource to be identified in a unique way.
///
/// Each [`Resource`] enum variant corresponds to a supported resource type
/// with specific included variables to uniquely identify all resources.
#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub enum Resource {
    /// File variant includes the absolute path of the corresponding file on the
    /// system as a String object.
    File(String),
    /// Stream variant includes the local socket address and peer socket address
    /// as a couple of SocketAddr objects.
    Stream(SocketAddr, SocketAddr),
    /// Process variant includes the pid as u32 and the starttime as u64 to
    /// properly handle possible pid recycling for different process instances.
    Process(u32, u64),
}
