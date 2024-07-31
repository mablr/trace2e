use std::fmt;
use std::net::SocketAddr;

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub enum Identifier {
    File(String),
    Stream(SocketAddr, SocketAddr),
    Process(u32)
}

impl fmt::Display for Identifier {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Identifier::File(path) => write!(f, "{}", path),
            Identifier::Stream(local_socket, peer_socket) => write!(f, "[{}-{}]", local_socket, peer_socket),
            Identifier::Process(pid) => write!(f, "{}", pid),
        }
    }
}