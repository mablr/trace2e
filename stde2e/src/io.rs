use std::process;

use crate::middleware::{Flow, IoInfo, IoResult, GRPC_CLIENT, TOKIO_RUNTIME};

fn middleware_request(fd: i32, flow: i32) -> Result<u64, Box<dyn std::error::Error>> {
    let mut client = GRPC_CLIENT.clone();

    let request = tonic::Request::new(IoInfo {
        process_id: process::id(),
        file_descriptor: fd,
        flow,
    });

    match TOKIO_RUNTIME.block_on(client.io_request(request)) {
        Ok(response) => Ok(response.into_inner().id),
        Err(_) => Err(Box::new(std::io::Error::from(
            std::io::ErrorKind::PermissionDenied,
        ))),
    }
}

fn middleware_report(fd: i32, grant_id: u64, result: bool) -> std::io::Result<()> {
    let mut client = GRPC_CLIENT.clone();

    let request = tonic::Request::new(IoResult {
        process_id: process::id(),
        file_descriptor: fd,
        grant_id,
        result,
    });

    match TOKIO_RUNTIME.block_on(client.io_report(request)) {
        Ok(_) => Ok(()),
        Err(_) => Err(std::io::Error::from(std::io::ErrorKind::Other)),
    }
}

pub trait Read: std::io::Read + std::os::fd::AsRawFd {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if let Ok(grant_id) = middleware_request(self.as_raw_fd(), Flow::Input.into()) {
            let result = std::io::Read::read(self, buf);
            middleware_report(self.as_raw_fd(), grant_id, result.is_ok())?;
            result
        } else {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        }
    }

    fn read_vectored(&mut self, bufs: &mut [std::io::IoSliceMut<'_>]) -> std::io::Result<usize> {
        if let Ok(grant_id) = middleware_request(self.as_raw_fd(), Flow::Input.into()) {
            let result = std::io::Read::read_vectored(self, bufs);
            middleware_report(self.as_raw_fd(), grant_id, result.is_ok())?;
            result
        } else {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        }
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> std::io::Result<usize> {
        if let Ok(grant_id) = middleware_request(self.as_raw_fd(), Flow::Input.into()) {
            let result = std::io::Read::read_to_end(self, buf);
            middleware_report(self.as_raw_fd(), grant_id, result.is_ok())?;
            result
        } else {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        }
    }

    fn read_to_string(&mut self, buf: &mut String) -> std::io::Result<usize> {
        if let Ok(grant_id) = middleware_request(self.as_raw_fd(), Flow::Input.into()) {
            let result = std::io::Read::read_to_string(self, buf);
            middleware_report(self.as_raw_fd(), grant_id, result.is_ok())?;
            result
        } else {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        }
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> std::io::Result<()> {
        if let Ok(grant_id) = middleware_request(self.as_raw_fd(), Flow::Input.into()) {
            let result = std::io::Read::read_exact(self, buf);
            middleware_report(self.as_raw_fd(), grant_id, result.is_ok())?;
            result
        } else {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        }
    }
}

impl<R: std::io::Read + std::os::fd::AsRawFd> Read for R {}

pub trait Write: std::io::Write + std::os::fd::AsRawFd {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Ok(grant_id) = middleware_request(self.as_raw_fd(), Flow::Output.into()) {
            let result = std::io::Write::write(self, buf);
            middleware_report(self.as_raw_fd(), grant_id, result.is_ok())?;
            result
        } else {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if let Ok(grant_id) = middleware_request(self.as_raw_fd(), Flow::Output.into()) {
            let result = std::io::Write::flush(self);
            middleware_report(self.as_raw_fd(), grant_id, result.is_ok())?;
            result
        } else {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        }
    }

    fn write_vectored(&mut self, bufs: &[std::io::IoSlice<'_>]) -> std::io::Result<usize> {
        if let Ok(grant_id) = middleware_request(self.as_raw_fd(), Flow::Output.into()) {
            let result = std::io::Write::write_vectored(self, bufs);
            middleware_report(self.as_raw_fd(), grant_id, result.is_ok())?;
            result
        } else {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        }
    }

    fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        if let Ok(grant_id) = middleware_request(self.as_raw_fd(), Flow::Output.into()) {
            let result = std::io::Write::write_all(self, buf);
            middleware_report(self.as_raw_fd(), grant_id, result.is_ok())?;
            result
        } else {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        }
    }

    fn write_fmt(&mut self, fmt: std::fmt::Arguments<'_>) -> std::io::Result<()> {
        if let Ok(grant_id) = middleware_request(self.as_raw_fd(), Flow::Output.into()) {
            let result = std::io::Write::write_fmt(self, fmt);
            middleware_report(self.as_raw_fd(), grant_id, result.is_ok())?;
            result
        } else {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        }
    }
}

impl<W: std::io::Write + std::os::fd::AsRawFd> Write for W {}
