use std::net::SocketAddr;
use std::net::TcpListener as StdTcpListener;
use std::net::TcpStream as StdTcpStream;
use std::os::fd::AsRawFd;
use std::process::id;

use crate::middleware::{RemoteCt, GRPC_CLIENT, TOKIO_RUNTIME};

fn middleware_report(fd: i32, local_socket: String, peer_socket: String) {
    let mut client = GRPC_CLIENT.clone();

    let request = tonic::Request::new(RemoteCt {
        process_id: id(),
        file_descriptor: fd,
        local_socket,
        peer_socket,
    });

    let _ = TOKIO_RUNTIME.block_on(client.remote_enroll(request));
}

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
        middleware_report(
            tcp_stream.as_raw_fd(),
            tcp_stream.local_addr()?.to_string(),
            tcp_stream.peer_addr()?.to_string(),
        );
        Ok((tcp_stream, socket))
    }

    pub fn incoming(&self) -> std::net::Incoming<'_> {
        self.0.incoming()
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
        middleware_report(
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
        middleware_report(
            tcp_stream.as_raw_fd(),
            tcp_stream.local_addr()?.to_string(),
            tcp_stream.peer_addr()?.to_string(),
        );
        Ok(tcp_stream)
    }
}
