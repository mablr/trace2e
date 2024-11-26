//! Global identification mechanism for data containers.

use std::fmt;
use std::net::SocketAddr;

use std::sync::OnceLock;

use crate::m2m_service::m2m;

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

    pub fn new_process(pid: u32, starttime: u64, exe_path: String) -> Self {
        Self {
            node: MIDDLEWARE_ID
                .get_or_init(|| "localhost".to_string())
                .clone(),
            resource: Resource::Process(pid, starttime, exe_path),
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
            Resource::Process(pid, ..) => Some(pid),
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

impl From<m2m::Id> for Identifier {
    fn from(id: m2m::Id) -> Self {
        Identifier {
            node: id.node,
            // Assuming the resource field is always for a file.
            resource: match id.resource.unwrap().variant.unwrap() {
                m2m::resource::Variant::File(file) => Resource::File(file.path),
                m2m::resource::Variant::Stream(stream) => Resource::Stream(
                    stream.local_socket.parse().unwrap(),
                    stream.peer_socket.parse().unwrap(),
                ),
                m2m::resource::Variant::Process(process) => {
                    Resource::Process(process.pid, process.starttime, process.exe_path)
                }
            },
        }
    }
}

impl From<Identifier> for m2m::Id {
    fn from(identifier_internal: Identifier) -> Self {
        m2m::Id {
            node: identifier_internal.node,
            resource: Some(m2m::Resource {
                variant: Some(match identifier_internal.resource {
                    Resource::File(path) => m2m::resource::Variant::File(m2m::File { path }),
                    Resource::Stream(local_socket, peer_socket) => {
                        m2m::resource::Variant::Stream(m2m::Stream {
                            local_socket: local_socket.to_string(),
                            peer_socket: peer_socket.to_string(),
                        })
                    }
                    Resource::Process(pid, starttime, exe_path) => {
                        m2m::resource::Variant::Process(m2m::Process {
                            pid,
                            starttime,
                            exe_path,
                        })
                    }
                }),
            }),
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
    /// Process variant includes the pid as u32, the starttime as u64 (to
    /// properly handle possible pid recycling for different process instances)
    /// and exe path as String.
    Process(u32, u64, String),
}

impl fmt::Display for Resource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Resource::File(path) => write!(f, "file://{}", path),
            Resource::Stream(local, peer) => write!(f, "stream://tcp;{};{}", local, peer),
            Resource::Process(pid, start_time, exe_path) => {
                write!(f, "process://{};{};{}", pid, start_time, exe_path)
            }
        }
    }
}

// Implement Display for Identifier
impl fmt::Display for Identifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.resource, self.node)
    }
}
