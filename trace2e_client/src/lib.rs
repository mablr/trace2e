use once_cell::sync::Lazy;
use std::{fs::canonicalize, path::Path, process::id};
use tokio::{runtime::Handle, task};
use tonic::transport::Channel;
#[cfg(feature = "benchmarking")]
use tracing::info;

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

// The URL for the gRPC service
const GRPC_URL: &str = "http://[::1]:8080";

pub(crate) static GRPC_CLIENT: Lazy<proto::p2m_client::P2mClient<Channel>> = Lazy::new(|| {
    let rt = &*TOKIO_RUNTIME;
    rt.block_on(proto::p2m_client::P2mClient::connect(GRPC_URL))
        .unwrap()
});

// Gets an appropriate gRPC client for the current runtime context
fn get_client() -> proto::p2m_client::P2mClient<Channel> {
    if Handle::try_current().is_ok() {
        // We're already in a Tokio runtime, use thread-local client for this runtime
        thread_local! {
            static RUNTIME_CLIENT: once_cell::unsync::OnceCell<proto::p2m_client::P2mClient<Channel>> = 
                once_cell::unsync::OnceCell::new();
        }
        
        RUNTIME_CLIENT.with(|cell| {
            cell.get_or_init(|| {
                task::block_in_place(|| {
                    Handle::current()
                        .block_on(proto::p2m_client::P2mClient::connect(GRPC_URL))
                        .unwrap()
                })
            })
            .clone()
        })
    } else {
        // Use the global client
        GRPC_CLIENT.clone()
    }
}

pub fn local_enroll(path: impl AsRef<Path>, fd: i32) {
    #[cfg(feature = "benchmarking")]
    info!(
        "[P2M] P-> local_enroll (PID: {}, FD: {}, Path: {})",
        id(),
        fd,
        canonicalize(path.as_ref())
            .unwrap()
            .to_str()
            .unwrap()
            .to_string()
    );

    let request = tonic::Request::new(proto::LocalCt {
        process_id: id(),
        file_descriptor: fd,
        path: canonicalize(path.as_ref())
            .unwrap()
            .to_str()
            .unwrap()
            .to_string(),
    });

    if let Ok(handle) = Handle::try_current() {
        task::block_in_place(move || {
            let mut client = get_client();
            let _ = handle.block_on(client.local_enroll(request));
        });
    } else {
        let mut client = get_client();
        let _ = TOKIO_RUNTIME.block_on(client.local_enroll(request));
    }

    #[cfg(feature = "benchmarking")]
    info!(
        "[P2M] P<- local_enroll (PID: {}, FD: {}, Path: {})",
        id(),
        fd,
        canonicalize(path.as_ref())
            .unwrap()
            .to_str()
            .unwrap()
            .to_string()
    );
}

pub fn remote_enroll(fd: i32, local_socket: String, peer_socket: String) {
    #[cfg(feature = "benchmarking")]
    info!(
        "[P2M] P-> remote_enroll (PID: {}, FD: {}, Stream: [{}-{}])",
        id(),
        fd,
        local_socket,
        peer_socket
    );

    let request = tonic::Request::new(proto::RemoteCt {
        process_id: id(),
        file_descriptor: fd,
        local_socket,
        peer_socket,
    });

    if let Ok(handle) = Handle::try_current() {
        task::block_in_place(move || {
            let mut client = get_client();
            let _ = handle.block_on(client.remote_enroll(request));
        });
    } else {
        let mut client = get_client();
        let _ = TOKIO_RUNTIME.block_on(client.remote_enroll(request));
    }

    #[cfg(feature = "benchmarking")]
    info!("[P2M] P<- remote_enroll (PID: {}, FD: {})", id(), fd);
}

pub fn io_request(fd: i32, flow: i32) -> Result<u64, Box<dyn std::error::Error>> {
    #[cfg(feature = "benchmarking")]
    info!(
        "[P2M] P-> io_request (PID: {}, FD: {}, Flow: {})",
        id(),
        fd,
        flow
    );

    let request = tonic::Request::new(proto::IoInfo {
        process_id: id(),
        file_descriptor: fd,
        flow,
    });

    let result = if let Ok(handle) = Handle::try_current() {
        match task::block_in_place(move || {
            let mut client = get_client();
            handle.block_on(client.io_request(request))
        }) {
            Ok(response) => Ok(response.into_inner().id),
            Err(_) => Err(Box::new(std::io::Error::from(
                std::io::ErrorKind::PermissionDenied,
            ))),
        }
    } else {
        let mut client = get_client();
        match TOKIO_RUNTIME.block_on(client.io_request(request)) {
            Ok(response) => Ok(response.into_inner().id),
            Err(_) => Err(Box::new(std::io::Error::from(
                std::io::ErrorKind::PermissionDenied,
            ))),
        }
    };

    #[cfg(feature = "benchmarking")]
    info!(
        "[P2M] P<- io_request (PID: {}, FD: {}, Flow: {})",
        id(),
        fd,
        flow
    );

    result.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
}

pub fn io_report(fd: i32, grant_id: u64, result: bool) -> std::io::Result<()> {
    #[cfg(feature = "benchmarking")]
    info!(
        "[P2M] P-> io_report (PID: {}, FD: {}, grant_id: {}, result: {})",
        id(),
        fd,
        grant_id,
        result
    );

    let request = tonic::Request::new(proto::IoResult {
        process_id: id(),
        file_descriptor: fd,
        grant_id,
        result,
    });

    let result = if let Ok(handle) = Handle::try_current() {
        match task::block_in_place(move || {
            let mut client = get_client();
            handle.block_on(client.io_report(request))
        }) {
            Ok(_) => Ok(()),
            Err(_) => Err(std::io::Error::from(std::io::ErrorKind::Other)),
        }
    } else {
        let mut client = get_client();
        match TOKIO_RUNTIME.block_on(client.io_report(request)) {
            Ok(_) => Ok(()),
            Err(_) => Err(std::io::Error::from(std::io::ErrorKind::Other)),
        }
    };

    #[cfg(feature = "benchmarking")]
    info!(
        "[P2M] P<- io_report (PID: {}, FD: {}, grant_id: {}, result: {})",
        id(),
        fd,
        grant_id,
        result.is_ok()
    );

    result
}
