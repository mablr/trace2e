use std::{fs::canonicalize, path::Path, process::id};
mod proto {
    tonic::include_proto!("trace2e");
}

mod grpc {
    use super::*;
    use once_cell::sync::Lazy;
    use tokio::{runtime::Handle, task};
    use tonic::transport::Channel;

    static TOKIO_RUNTIME: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
    });

    // The URL for the gRPC service
    const GRPC_URL: &str = "http://[::1]:8080";

    static GRPC_CLIENT: Lazy<proto::trace2e_grpc_client::Trace2eGrpcClient<Channel>> = Lazy::new(|| {
        let rt = &*TOKIO_RUNTIME;
        rt.block_on(proto::trace2e_grpc_client::Trace2eGrpcClient::connect(GRPC_URL))
            .unwrap()
    });

    // Gets an appropriate gRPC client for the current runtime context
    fn get_client() -> proto::trace2e_grpc_client::Trace2eGrpcClient<Channel> {
        if Handle::try_current().is_ok() {
            // We're already in a Tokio runtime, use thread-local client for this runtime
            thread_local! {
                static RUNTIME_CLIENT: once_cell::unsync::OnceCell<proto::trace2e_grpc_client::Trace2eGrpcClient<Channel>> =
                    once_cell::unsync::OnceCell::new();
            }

            RUNTIME_CLIENT.with(|cell| {
                cell.get_or_init(|| {
                    task::block_in_place(|| {
                        Handle::current()
                            .block_on(proto::trace2e_grpc_client::Trace2eGrpcClient::connect(GRPC_URL))
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
        let request = tonic::Request::new(proto::LocalCt {
            process_id: id() as i32,
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
                let _ = handle.block_on(client.p2m_local_enroll(request));
            });
        } else {
            let mut client = get_client();
            let _ = TOKIO_RUNTIME.block_on(client.p2m_local_enroll(request));
        }
    }

    pub fn remote_enroll(fd: i32, local_socket: String, peer_socket: String) {
        let request = tonic::Request::new(proto::RemoteCt {
            process_id: id() as i32,
            file_descriptor: fd,
            local_socket,
            peer_socket,
        });

        if let Ok(handle) = Handle::try_current() {
            task::block_in_place(move || {
                let mut client = get_client();
                let _ = handle.block_on(client.p2m_remote_enroll(request));
            });
        } else {
            let mut client = get_client();
            let _ = TOKIO_RUNTIME.block_on(client.p2m_remote_enroll(request));
        }
    }

    pub fn io_request(fd: i32, flow: i32) -> Result<u128, Box<dyn std::error::Error>> {
        let request = tonic::Request::new(proto::IoInfo {
            process_id: id() as i32,
            file_descriptor: fd,
            flow,
        });

        let result = if let Ok(handle) = Handle::try_current() {
            match task::block_in_place(move || {
                let mut client = get_client();
                handle.block_on(client.p2m_io_request(request))
            }) {
                Ok(response) => Ok(response.into_inner().id),
                Err(_) => Err(Box::new(std::io::Error::from(
                    std::io::ErrorKind::PermissionDenied,
                ))),
            }
        } else {
            let mut client = get_client();
            match TOKIO_RUNTIME.block_on(client.p2m_io_request(request)) {
                Ok(response) => Ok(response.into_inner().id),
                Err(_) => Err(Box::new(std::io::Error::from(
                    std::io::ErrorKind::PermissionDenied,
                ))),
            }
        };
        result
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
            .map(|id| id.parse::<u128>().unwrap())
    }

    pub fn io_report(fd: i32, grant_id: u128, result: bool) -> std::io::Result<()> {
        let request = tonic::Request::new(proto::IoResult {
            process_id: id() as i32,
            file_descriptor: fd,
            grant_id: grant_id.to_string(),
            result,
        });

        let result = if let Ok(handle) = Handle::try_current() {
            match task::block_in_place(move || {
                let mut client = get_client();
                handle.block_on(client.p2m_io_report(request))
            }) {
                Ok(_) => Ok(()),
                Err(_) => Err(std::io::Error::from(std::io::ErrorKind::Other)),
            }
        } else {
            let mut client = get_client();
            match TOKIO_RUNTIME.block_on(client.p2m_io_report(request)) {
                Ok(_) => Ok(()),
                Err(_) => Err(std::io::Error::from(std::io::ErrorKind::Other)),
            }
        };

        result
    }
}

pub use proto::Flow;

pub use grpc::{io_report, io_request, local_enroll, remote_enroll};
