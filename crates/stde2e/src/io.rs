use trace2e_client::p2m::{io_report, io_request};
use trace2e_client::primitives::Flow;

pub struct BufReader<R>(std::io::BufReader<R>);

pub trait Read: std::io::Read {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        std::io::Read::read(self, buf)
    }

    fn read_vectored(&mut self, bufs: &mut [std::io::IoSliceMut<'_>]) -> std::io::Result<usize> {
        std::io::Read::read_vectored(self, bufs)
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> std::io::Result<usize> {
        std::io::Read::read_to_end(self, buf)
    }

    fn read_to_string(&mut self, buf: &mut String) -> std::io::Result<usize> {
        std::io::Read::read_to_string(self, buf)
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> std::io::Result<()> {
        std::io::Read::read_exact(self, buf)
    }

    fn by_ref(&mut self) -> &mut Self
    where
        Self: Sized,
    {
        std::io::Read::by_ref(self)
    }

    fn bytes(self) -> std::io::Bytes<Self>
    where
        Self: Sized,
    {
        std::io::Read::bytes(self)
    }

    fn chain<R: std::io::Read>(self, next: R) -> std::io::Chain<Self, R>
    where
        Self: Sized,
    {
        std::io::Read::chain(self, next)
    }

    fn take(self, limit: u64) -> std::io::Take<Self>
    where
        Self: Sized,
    {
        std::io::Read::take(self, limit)
    }
}

impl<R: std::io::Read + std::os::fd::AsRawFd> Read for R {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if let Ok(grant_id) = io_request(self.as_raw_fd(), Flow::Input.into()) {
            let result = std::io::Read::read(self, buf);
            io_report(self.as_raw_fd(), grant_id, result.is_ok())?;
            result
        } else {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        }
    }

    fn read_vectored(&mut self, bufs: &mut [std::io::IoSliceMut<'_>]) -> std::io::Result<usize> {
        if let Ok(grant_id) = io_request(self.as_raw_fd(), Flow::Input.into()) {
            let result = std::io::Read::read_vectored(self, bufs);
            io_report(self.as_raw_fd(), grant_id, result.is_ok())?;
            result
        } else {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        }
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> std::io::Result<usize> {
        if let Ok(grant_id) = io_request(self.as_raw_fd(), Flow::Input.into()) {
            let result = std::io::Read::read_to_end(self, buf);
            io_report(self.as_raw_fd(), grant_id, result.is_ok())?;
            result
        } else {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        }
    }

    fn read_to_string(&mut self, buf: &mut String) -> std::io::Result<usize> {
        if let Ok(grant_id) = io_request(self.as_raw_fd(), Flow::Input.into()) {
            let result = std::io::Read::read_to_string(self, buf);
            io_report(self.as_raw_fd(), grant_id, result.is_ok())?;
            result
        } else {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        }
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> std::io::Result<()> {
        if let Ok(grant_id) = io_request(self.as_raw_fd(), Flow::Input.into()) {
            let result = std::io::Read::read_exact(self, buf);
            io_report(self.as_raw_fd(), grant_id, result.is_ok())?;
            result
        } else {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        }
    }
}

pub trait Write: std::io::Write {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        std::io::Write::write(self, buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        std::io::Write::flush(self)
    }

    fn write_vectored(&mut self, bufs: &[std::io::IoSlice<'_>]) -> std::io::Result<usize> {
        std::io::Write::write_vectored(self, bufs)
    }

    fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        std::io::Write::write_all(self, buf)
    }

    fn write_fmt(&mut self, fmt: std::fmt::Arguments<'_>) -> std::io::Result<()> {
        std::io::Write::write_fmt(self, fmt)
    }

    fn by_ref(&mut self) -> &mut Self
    where
        Self: Sized,
    {
        std::io::Write::by_ref(self)
    }
}

impl<W: std::io::Write + std::os::fd::AsRawFd> Write for W {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Ok(grant_id) = io_request(self.as_raw_fd(), Flow::Output.into()) {
            let result = std::io::Write::write(self, buf);
            io_report(self.as_raw_fd(), grant_id, result.is_ok())?;
            result
        } else {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        }
    }

    fn write_vectored(&mut self, bufs: &[std::io::IoSlice<'_>]) -> std::io::Result<usize> {
        if let Ok(grant_id) = io_request(self.as_raw_fd(), Flow::Output.into()) {
            let result = std::io::Write::write_vectored(self, bufs);
            io_report(self.as_raw_fd(), grant_id, result.is_ok())?;
            result
        } else {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        }
    }

    fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        if let Ok(grant_id) = io_request(self.as_raw_fd(), Flow::Output.into()) {
            let result = std::io::Write::write_all(self, buf);
            io_report(self.as_raw_fd(), grant_id, result.is_ok())?;
            result
        } else {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        }
    }

    fn write_fmt(&mut self, fmt: std::fmt::Arguments<'_>) -> std::io::Result<()> {
        if let Ok(grant_id) = io_request(self.as_raw_fd(), Flow::Output.into()) {
            let result = std::io::Write::write_fmt(self, fmt);
            io_report(self.as_raw_fd(), grant_id, result.is_ok())?;
            result
        } else {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        }
    }
}

pub trait BufRead: std::io::BufRead {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        std::io::BufRead::fill_buf(self)
    }

    fn consume(&mut self, amt: usize) {
        std::io::BufRead::consume(self, amt)
    }

    fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> std::io::Result<usize> {
        std::io::BufRead::read_until(self, byte, buf)
    }

    fn read_line(&mut self, buf: &mut String) -> std::io::Result<usize> {
        std::io::BufRead::read_line(self, buf)
    }

    fn split(self, byte: u8) -> std::io::Split<Self>
    where
        Self: Sized,
    {
        std::io::BufRead::split(self, byte)
    }

    fn lines(self) -> std::io::Lines<Self>
    where
        Self: Sized,
    {
        std::io::BufRead::lines(self)
    }
}

impl<T: std::io::BufRead + std::os::fd::AsRawFd> BufRead for std::io::Take<T> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        let reader_raw_fd = self.get_ref().as_raw_fd();
        if let Ok(grant_id) = io_request(reader_raw_fd, Flow::Input.into()) {
            let result = std::io::BufRead::fill_buf(self);
            io_report(reader_raw_fd, grant_id, result.is_ok())?;
            result
        } else {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        }
    }
}

impl<R: ?Sized + std::io::Read + std::os::fd::AsRawFd> BufRead for std::io::BufReader<R> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        let reader_raw_fd = self.get_ref().as_raw_fd();
        if let Ok(grant_id) = io_request(reader_raw_fd, Flow::Input.into()) {
            let result = std::io::BufRead::fill_buf(self);
            io_report(reader_raw_fd, grant_id, result.is_ok())?;
            result
        } else {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        }
    }
}
