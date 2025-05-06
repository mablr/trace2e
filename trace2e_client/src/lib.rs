use std::{fs::canonicalize, path::Path, process::id};
mod proto {
    tonic::include_proto!("p2m_api");
}

#[cfg(not(feature = "dbus"))]
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

    static GRPC_CLIENT: Lazy<proto::p2m_client::P2mClient<Channel>> = Lazy::new(|| {
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
    }

    pub fn remote_enroll(fd: i32, local_socket: String, peer_socket: String) {
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
    }

    pub fn io_request(fd: i32, flow: i32) -> Result<u64, Box<dyn std::error::Error>> {
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
        result.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    pub fn io_report(fd: i32, grant_id: u64, result: bool) -> std::io::Result<()> {
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
            
        result
    }
}

#[cfg(feature = "dbus")]
mod dbus {
    use super::*;
    use once_cell::sync::Lazy;
    use zbus::proxy;

    #[proxy(
        interface = "org.trace2e.P2m",
        default_service = "org.trace2e.P2m",
        default_path = "/org/trace2e/P2m"
    )]
    trait P2m {
        async fn local_enroll(&self, process_id: u32, file_descriptor: i32, path: String) -> zbus::Result<()>;
        async fn remote_enroll(&self, process_id: u32, file_descriptor: i32, local_socket: String, peer_socket: String) -> zbus::Result<()>;
        async fn io_request(&self, process_id: u32, file_descriptor: i32, flow: i32) -> zbus::Result<u64>;
        async fn io_report(&self, process_id: u32, file_descriptor: i32, grant_id: u64, result: bool) -> zbus::Result<()>;
    }

    static DBUS_CONNECTION: Lazy<zbus::blocking::Connection> = Lazy::new(|| {
        zbus::blocking::Connection::session().unwrap()
    });

    static DBUS_PROXY: Lazy<P2mProxyBlocking> = Lazy::new(|| {
        P2mProxyBlocking::new(&*DBUS_CONNECTION).unwrap()
    });

    pub fn local_enroll(path: impl AsRef<Path>, fd: i32) {
        let path = canonicalize(path.as_ref())
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let process_id = id();
        let _ = DBUS_PROXY.local_enroll(process_id, fd, path.clone());
    }

    pub fn remote_enroll(fd: i32, local_socket: String, peer_socket: String) {
        let process_id = id();
        let _ = DBUS_PROXY.remote_enroll(process_id, fd, local_socket, peer_socket);
    }

    pub fn io_request(fd: i32, flow: i32) -> Result<u64, Box<dyn std::error::Error>> {
        match DBUS_PROXY.io_request(id(), fd, flow) {
            Ok(grant_id) => Ok(grant_id),
            Err(_) => Err(Box::new(std::io::Error::from(
                std::io::ErrorKind::PermissionDenied,
            ))) as Box<dyn std::error::Error>
        }
    }

    pub fn io_report(fd: i32, grant_id: u64, result: bool) -> std::io::Result<()> {
        match DBUS_PROXY.io_report(id(), fd, grant_id, result) {
            Ok(_) => Ok(()),
            Err(_) => Err(std::io::Error::from(std::io::ErrorKind::Other)),
        }
    }
}


pub use proto::Flow;

#[cfg(not(feature = "dbus"))]
pub use grpc::{local_enroll, remote_enroll, io_request, io_report};

#[cfg(feature = "dbus")]
pub use dbus::{local_enroll, remote_enroll, io_request, io_report};
