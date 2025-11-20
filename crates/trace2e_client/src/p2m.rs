use once_cell::{sync::Lazy, unsync::OnceCell};
use std::{fs::canonicalize, path::Path, process::id};
use tokio::{
    runtime::Handle,
    task::{self, block_in_place},
};
use tonic::transport::Channel;

use trace2e_core::transport::grpc::proto;

static TOKIO_RUNTIME: Lazy<tokio::runtime::Runtime> =
    Lazy::new(|| tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap());

// Get the gRPC URL from environment variable or use default
fn get_grpc_url() -> String {
    std::env::var("TRACE2E_MIDDLEWARE_URL").unwrap_or_else(|_| "http://[::1]:50051".to_string())
}

static P2M_CLIENT: Lazy<proto::p2m_client::P2mClient<Channel>> = Lazy::new(|| {
    let rt = &*TOKIO_RUNTIME;
    rt.block_on(proto::p2m_client::P2mClient::connect(get_grpc_url())).unwrap()
});

// Gets an appropriate P2M client for the current runtime context
fn get_p2m_client() -> proto::p2m_client::P2mClient<Channel> {
    if Handle::try_current().is_ok() {
        // We're already in a Tokio runtime, use thread-local client for this runtime
        thread_local! {
            static RUNTIME_CLIENT: OnceCell<proto::p2m_client::P2mClient<Channel>> =
                const { OnceCell::new() };
        }

        RUNTIME_CLIENT.with(|cell| {
            cell.get_or_init(|| {
                block_in_place(|| {
                    Handle::current()
                        .block_on(proto::p2m_client::P2mClient::connect(get_grpc_url()))
                        .unwrap()
                })
            })
            .clone()
        })
    } else {
        // Use the global client
        P2M_CLIENT.clone()
    }
}

pub fn local_enroll(path: impl AsRef<Path>, fd: i32) {
    let request = tonic::Request::new(proto::messages::LocalCt {
        process_id: id() as i32,
        file_descriptor: fd,
        path: canonicalize(path.as_ref()).unwrap().to_str().unwrap().to_string(),
    });

    if let Ok(handle) = Handle::try_current() {
        task::block_in_place(move || {
            let mut client = get_p2m_client();
            let _ = handle.block_on(client.p2m_local_enroll(request));
        });
    } else {
        let mut client = get_p2m_client();
        let _ = TOKIO_RUNTIME.block_on(client.p2m_local_enroll(request));
    }
}

pub fn remote_enroll(fd: i32, local_socket: String, peer_socket: String) {
    let request = tonic::Request::new(proto::messages::RemoteCt {
        process_id: id() as i32,
        file_descriptor: fd,
        local_socket,
        peer_socket,
    });

    if let Ok(handle) = Handle::try_current() {
        block_in_place(move || {
            let mut client = get_p2m_client();
            let _ = handle.block_on(client.p2m_remote_enroll(request));
        });
    } else {
        let mut client = get_p2m_client();
        let _ = TOKIO_RUNTIME.block_on(client.p2m_remote_enroll(request));
    }
}

pub fn io_request(fd: i32, flow: i32) -> Result<u128, Box<dyn std::error::Error>> {
    let request = tonic::Request::new(proto::messages::IoInfo {
        process_id: id() as i32,
        file_descriptor: fd,
        flow,
    });

    let result = if let Ok(handle) = Handle::try_current() {
        match task::block_in_place(move || {
            let mut client = get_p2m_client();
            handle.block_on(client.p2m_io_request(request))
        }) {
            Ok(response) => Ok(response.into_inner().id),
            Err(_) => Err(Box::new(std::io::Error::from(std::io::ErrorKind::PermissionDenied))),
        }
    } else {
        let mut client = get_p2m_client();
        match TOKIO_RUNTIME.block_on(client.p2m_io_request(request)) {
            Ok(response) => Ok(response.into_inner().id),
            Err(_) => Err(Box::new(std::io::Error::from(std::io::ErrorKind::PermissionDenied))),
        }
    };
    result
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
        .map(|id| id.parse::<u128>().unwrap())
}

pub fn io_report(fd: i32, grant_id: u128, result: bool) -> std::io::Result<()> {
    let request = tonic::Request::new(proto::messages::IoResult {
        process_id: id() as i32,
        file_descriptor: fd,
        grant_id: grant_id.to_string(),
        result,
    });

    if let Ok(handle) = Handle::try_current() {
        match block_in_place(move || {
            let mut client = get_p2m_client();
            handle.block_on(client.p2m_io_report(request))
        }) {
            Ok(_) => Ok(()),
            Err(_) => Err(std::io::Error::from(std::io::ErrorKind::Other)),
        }
    } else {
        let mut client = get_p2m_client();
        match TOKIO_RUNTIME.block_on(client.p2m_io_report(request)) {
            Ok(_) => Ok(()),
            Err(_) => Err(std::io::Error::from(std::io::ErrorKind::Other)),
        }
    }
}
