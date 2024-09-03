//! Global identification mechanism for data containers.

use std::fmt;
use std::net::SocketAddr;

/// Container identification object.
/// 
/// Enum that is instantiated when a container is created. It allows any type of
/// container to be identified in a unique way.
///
/// Each [`Identifier`] enum variant corresponds to a supported container type 
/// with specific included variables to uniquely identify all containers.
#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub enum Identifier {
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

impl fmt::Display for Identifier {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Identifier::File(path) => write!(f, "{}", path),
            Identifier::Stream(local_socket, peer_socket) => {
                write!(f, "{}<->{}", local_socket, peer_socket)
            }
            Identifier::Process(pid, _) => write!(f, "{}", pid),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

    use super::Identifier;

    #[test]
    fn unit_identifier_display_file() {
        let path = "/path/to/file".to_string();
        let file = Identifier::File(path.clone());
        assert_eq!(format!("{}", file), path);
    }

    #[test]
    fn unit_identifier_display_stream() {
        let stream_v4 = Identifier::Stream(
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
        );
        let stream_v6 = Identifier::Stream(
            SocketAddr::new(
                IpAddr::V6(Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 1)),
                12312,
            ),
            SocketAddr::new(IpAddr::V6(Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 2)), 8081),
        );
        assert_eq!(format!("{}", stream_v4), "127.0.0.1:12312<->127.0.0.1:8081");
        assert_eq!(format!("{}", stream_v6), "[fc00::1]:12312<->[fc00::2]:8081");
    }

    #[test]
    fn unit_identifier_display_process() {
        let pid = 1;
        let process = Identifier::Process(pid, 10000);
        let process_recycled_pid = Identifier::Process(pid, 10001);
        assert_eq!(format!("{}", process), format!("{}", pid));
        assert_eq!(format!("{}", process_recycled_pid), format!("{}", pid));
    }
}
