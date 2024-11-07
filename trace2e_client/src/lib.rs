use once_cell::sync::Lazy;
use std::{fs::canonicalize, path::Path, process::id};
use tokio::{runtime::Handle, task};
use tonic::transport::Channel;
mod proto {
    tonic::include_proto!("p2m_api");
}

pub use proto::Flow;

pub(crate) static TOKIO_RUNTIME: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
});

pub(crate) static GRPC_CLIENT: Lazy<proto::p2m_client::P2mClient<Channel>> = Lazy::new(|| {
    let rt = &*TOKIO_RUNTIME;
    rt.block_on(proto::p2m_client::P2mClient::connect("http://[::1]:8080"))
        .unwrap()
});

pub fn local_enroll(path: impl AsRef<Path>, fd: i32) {
    let request = tonic::Request::new(proto::LocalCt {
        process_id: id(),
        file_descriptor: fd,
        path: canonicalize(path).unwrap().display().to_string(),
    });

    if let Ok(handle) = Handle::try_current() {
        let _ = task::block_in_place(move || {
            // Workaround to avoid Lazy poisoning
            let mut client = handle
                .block_on(proto::p2m_client::P2mClient::connect("http://[::1]:8080"))
                .unwrap();
            handle.block_on(client.local_enroll(request))
        });
    } else {
        let mut client = GRPC_CLIENT.clone();
        let _ = TOKIO_RUNTIME.block_on(client.local_enroll(request));
    }
}

pub fn remote_enroll(fd: i32, local_socket: String, peer_socket: String) {
    let request = tonic::Request::new(proto::RemoteCt {
        process_id: id(),
        file_descriptor: fd,
        local_socket,
        peer_socket,
    });

    if let Ok(handle) = Handle::try_current() {
        let _ = task::block_in_place(move || {
            // Workaround to avoid Lazy poisoning
            let mut client = handle
                .block_on(proto::p2m_client::P2mClient::connect("http://[::1]:8080"))
                .unwrap();
            handle.block_on(client.remote_enroll(request))
        });
    } else {
        let mut client = GRPC_CLIENT.clone();
        let _ = TOKIO_RUNTIME.block_on(client.remote_enroll(request));
    }
}

pub fn io_request(fd: i32, flow: i32) -> Result<u64, Box<dyn std::error::Error>> {
    let request = tonic::Request::new(proto::IoInfo {
        process_id: id(),
        file_descriptor: fd,
        flow,
    });

    if let Ok(handle) = Handle::try_current() {
        match task::block_in_place(move || {
            // Workaround to avoid Lazy poisoning
            let mut client = Handle::current()
                .block_on(proto::p2m_client::P2mClient::connect("http://[::1]:8080"))
                .unwrap();
            handle.block_on(client.io_request(request))
        }) {
            Ok(response) => Ok(response.into_inner().id),
            Err(_) => Err(Box::new(std::io::Error::from(
                std::io::ErrorKind::PermissionDenied,
            ))),
        }
    } else {
        let mut client = GRPC_CLIENT.clone();
        match TOKIO_RUNTIME.block_on(client.io_request(request)) {
            Ok(response) => Ok(response.into_inner().id),
            Err(_) => Err(Box::new(std::io::Error::from(
                std::io::ErrorKind::PermissionDenied,
            ))),
        }
    }
}

pub fn io_report(fd: i32, grant_id: u64, result: bool) -> std::io::Result<()> {
    let request = tonic::Request::new(proto::IoResult {
        process_id: id(),
        file_descriptor: fd,
        grant_id,
        result,
    });

    if let Ok(handle) = Handle::try_current() {
        match task::block_in_place(move || {
            // Workaround to avoid Lazy poisoning
            let mut client = Handle::current()
                .block_on(proto::p2m_client::P2mClient::connect("http://[::1]:8080"))
                .unwrap();
            handle.block_on(client.io_report(request))
        }) {
            Ok(_) => Ok(()),
            Err(_) => Err(std::io::Error::from(std::io::ErrorKind::Other)),
        }
    } else {
        let mut client = GRPC_CLIENT.clone();
        match TOKIO_RUNTIME.block_on(client.io_report(request)) {
            Ok(_) => Ok(()),
            Err(_) => Err(std::io::Error::from(std::io::ErrorKind::Other)),
        }
    }
}
