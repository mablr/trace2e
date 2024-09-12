//! Global identification mechanism for data containers.

use std::net::SocketAddr;

use std::sync::OnceLock;

pub static MIDDLEWARE_ID: OnceLock<String> = OnceLock::new();

/// Global ressource identification object.
///
/// Structure associating [`MIDDLEWARE_ID`] with a local ressource
/// identification object to allow decentralized identification.
#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub struct Identifier {
    node: String,
    ressource: Ressource,
}

impl Identifier {
    pub fn new_file(path: String) -> Self {
        Self {
            node: MIDDLEWARE_ID
                .get_or_init(|| "localhost".to_string())
                .clone(),
            ressource: Ressource::File(path),
        }
    }

    pub fn new_stream(local_socket: SocketAddr, peer_socket: SocketAddr) -> Self {
        Self {
            node: MIDDLEWARE_ID
                .get_or_init(|| "localhost".to_string())
                .clone(),
            ressource: Ressource::Stream(local_socket, peer_socket),
        }
    }

    pub fn new_process(pid: u32, starttime: u64) -> Self {
        Self {
            node: MIDDLEWARE_ID
                .get_or_init(|| "localhost".to_string())
                .clone(),
            ressource: Ressource::Process(pid, starttime),
        }
    }

    pub fn is_file(&self) -> Option<&String> {
        match &self.ressource {
            Ressource::File(path) => Some(&path),
            _ => None,
        }
    }

    pub fn is_process(&self) -> Option<u32> {
        match self.ressource {
            Ressource::Process(pid, _) => Some(pid),
            _ => None,
        }
    }

    pub fn is_stream(&self) -> Option<(SocketAddr, SocketAddr)> {
        match self.ressource {
            Ressource::Stream(local_socket, peer_socket) => Some((local_socket, peer_socket)),
            _ => None,
        }
    }
}

/// Local ressource identification object.
///
/// Enum that is instantiated when a ressource is enrolled. It allows any type of
/// ressource to be identified in a unique way.
///
/// Each [`Ressource`] enum variant corresponds to a supported ressource type
/// with specific included variables to uniquely identify all ressources.
#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub enum Ressource {
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
