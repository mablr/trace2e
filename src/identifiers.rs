use std::fmt;
use std::net::SocketAddr;

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub enum Identifier {
    File(String),
    Stream(SocketAddr, SocketAddr),
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
    fn identifier_display_file() {
        let path = "/path/to/file".to_string();
        let file = Identifier::File(path.clone());
        assert_eq!(format!("{}", file), path);
    }

    #[test]
    fn identifier_display_stream() {
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
    fn identifier_display_process() {
        let pid = 1;
        let process = Identifier::Process(pid, 10000);
        let process_recycled_pid = Identifier::Process(pid, 10001);
        assert_eq!(format!("{}", process), format!("{}", pid));
        assert_eq!(format!("{}", process_recycled_pid), format!("{}", pid));
    }
}
