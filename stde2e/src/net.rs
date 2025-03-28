use std::net::SocketAddr;
use std::net::TcpListener as StdTcpListener;
use std::net::TcpStream as StdTcpStream;
use std::os::fd::AsRawFd;

use trace2e_client::remote_enroll;

pub struct TcpListener(StdTcpListener);

impl TcpListener {
    pub fn bind<A: std::net::ToSocketAddrs>(addr: A) -> std::io::Result<TcpListener> {
        match StdTcpListener::bind(addr) {
            Ok(tcp_listener) => Ok(TcpListener(tcp_listener)),
            Err(e) => Err(e),
        }
    }

    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.0.local_addr()
    }

    pub fn try_clone(&self) -> std::io::Result<TcpListener> {
        match self.0.try_clone() {
            Ok(tcp_listener) => Ok(TcpListener(tcp_listener)),
            Err(e) => Err(e),
        }
    }

    pub fn accept(&self) -> std::io::Result<(StdTcpStream, SocketAddr)> {
        let (tcp_stream, socket) = self.0.accept()?;
        remote_enroll(
            tcp_stream.as_raw_fd(),
            tcp_stream.local_addr()?.to_string(),
            tcp_stream.peer_addr()?.to_string(),
        );
        Ok((tcp_stream, socket))
    }
    pub fn incoming(&self) -> impl Iterator<Item = std::io::Result<StdTcpStream>> + '_ {
        self.0.incoming().map(|stream_result| {
            stream_result.map(|stream| {
                remote_enroll(
                    stream.as_raw_fd(),
                    stream.local_addr().unwrap().to_string(),
                    stream.peer_addr().unwrap().to_string(),
                );
                stream
            })
        })
    }

    pub fn set_ttl(&self, ttl: u32) -> std::io::Result<()> {
        self.0.set_ttl(ttl)
    }

    pub fn ttl(&self) -> std::io::Result<u32> {
        self.0.ttl()
    }

    pub fn take_error(&self) -> std::io::Result<Option<std::io::Error>> {
        self.0.take_error()
    }

    pub fn set_nonblocking(&self, nonblocking: bool) -> std::io::Result<()> {
        self.0.set_nonblocking(nonblocking)
    }
}

pub struct TcpStream;

impl TcpStream {
    pub fn connect<A: std::net::ToSocketAddrs>(addr: A) -> std::io::Result<StdTcpStream> {
        let tcp_stream = StdTcpStream::connect(addr)?;
        remote_enroll(
            tcp_stream.as_raw_fd(),
            tcp_stream.local_addr()?.to_string(),
            tcp_stream.peer_addr()?.to_string(),
        );
        Ok(tcp_stream)
    }
    pub fn connect_timeout<A>(
        addr: &SocketAddr,
        timeout: std::time::Duration,
    ) -> std::io::Result<StdTcpStream> {
        let tcp_stream = StdTcpStream::connect_timeout(addr, timeout)?;
        remote_enroll(
            tcp_stream.as_raw_fd(),
            tcp_stream.local_addr()?.to_string(),
            tcp_stream.peer_addr()?.to_string(),
        );
        Ok(tcp_stream)
    }
}
