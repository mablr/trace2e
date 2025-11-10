use std::{fs::canonicalize, path::Path, process::id};
mod proto {
    tonic::include_proto!("trace2e");
    pub mod primitives {
        tonic::include_proto!("trace2e.primitives");
    }
    pub mod messages {
        tonic::include_proto!("trace2e.messages");
    }
}

mod grpc {
    use once_cell::{sync::Lazy, unsync::OnceCell};
    use tokio::{
        runtime::Handle,
        task::{self, block_in_place},
    };
    use tonic::transport::Channel;

    use super::*;

    static TOKIO_RUNTIME: Lazy<tokio::runtime::Runtime> =
        Lazy::new(|| tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap());

    // The URL for the gRPC service
    const GRPC_URL: &str = "http://[::1]:50051";

    static GRPC_CLIENT: Lazy<proto::p2m_client::P2mClient<Channel>> = Lazy::new(|| {
        let rt = &*TOKIO_RUNTIME;
        rt.block_on(proto::p2m_client::P2mClient::connect(GRPC_URL)).unwrap()
    });

    #[cfg(feature = "o2m")]
    static O2M_CLIENT: Lazy<proto::o2m_client::O2mClient<Channel>> = Lazy::new(|| {
        let rt = &*TOKIO_RUNTIME;
        rt.block_on(proto::o2m_client::O2mClient::connect(GRPC_URL)).unwrap()
    });

    // Gets an appropriate gRPC client for the current runtime context
    fn get_client() -> proto::p2m_client::P2mClient<Channel> {
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

    #[cfg(feature = "o2m")]
    fn get_o2m_client() -> proto::o2m_client::O2mClient<Channel> {
        if Handle::try_current().is_ok() {
            // We're already in a Tokio runtime, use thread-local client for this runtime
            thread_local! {
                static RUNTIME_O2M_CLIENT: OnceCell<proto::o2m_client::O2mClient<Channel>> =
                    const { OnceCell::new() };
            }

            RUNTIME_O2M_CLIENT.with(|cell| {
                cell.get_or_init(|| {
                    block_in_place(|| {
                        Handle::current()
                            .block_on(proto::o2m_client::O2mClient::connect(GRPC_URL))
                            .unwrap()
                    })
                })
                .clone()
            })
        } else {
            // Use the global client
            O2M_CLIENT.clone()
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
                let mut client = get_client();
                let _ = handle.block_on(client.p2m_local_enroll(request));
            });
        } else {
            let mut client = get_client();
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
                let mut client = get_client();
                let _ = handle.block_on(client.p2m_remote_enroll(request));
            });
        } else {
            let mut client = get_client();
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
                let mut client = get_client();
                handle.block_on(client.p2m_io_request(request))
            }) {
                Ok(response) => Ok(response.into_inner().id),
                Err(_) => Err(Box::new(std::io::Error::from(std::io::ErrorKind::PermissionDenied))),
            }
        } else {
            let mut client = get_client();
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
        }
    }

    // O2M (Operator to Middleware) functions
    #[cfg(feature = "o2m")]
    pub fn get_policies(
        resources: Vec<proto::primitives::Resource>,
    ) -> Result<Vec<proto::primitives::MappedLocalizedPolicy>, Box<dyn std::error::Error>> {
        let request = tonic::Request::new(proto::messages::GetPoliciesRequest { resources });

        if let Ok(handle) = Handle::try_current() {
            match task::block_in_place(move || {
                let mut client = get_o2m_client();
                handle.block_on(client.o2m_get_policies(request))
            }) {
                Ok(response) => Ok(response.into_inner().policies),
                Err(e) => Err(Box::new(e) as Box<dyn std::error::Error>),
            }
        } else {
            let mut client = get_o2m_client();
            match TOKIO_RUNTIME.block_on(client.o2m_get_policies(request)) {
                Ok(response) => Ok(response.into_inner().policies),
                Err(e) => Err(Box::new(e) as Box<dyn std::error::Error>),
            }
        }
    }

    #[cfg(feature = "o2m")]
    pub fn set_policy(
        resource: proto::primitives::Resource,
        policy: proto::primitives::Policy,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let request = tonic::Request::new(proto::messages::SetPolicyRequest {
            resource: Some(resource),
            policy: Some(policy),
        });

        if let Ok(handle) = Handle::try_current() {
            match block_in_place(move || {
                let mut client = get_o2m_client();
                handle.block_on(client.o2m_set_policy(request))
            }) {
                Ok(_) => Ok(()),
                Err(e) => Err(Box::new(e) as Box<dyn std::error::Error>),
            }
        } else {
            let mut client = get_o2m_client();
            match TOKIO_RUNTIME.block_on(client.o2m_set_policy(request)) {
                Ok(_) => Ok(()),
                Err(e) => Err(Box::new(e) as Box<dyn std::error::Error>),
            }
        }
    }

    #[cfg(feature = "o2m")]
    pub fn set_confidentiality(
        resource: proto::primitives::Resource,
        confidentiality: i32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let request = tonic::Request::new(proto::messages::SetConfidentialityRequest {
            resource: Some(resource),
            confidentiality,
        });

        if let Ok(handle) = Handle::try_current() {
            match block_in_place(move || {
                let mut client = get_o2m_client();
                handle.block_on(client.o2m_set_confidentiality(request))
            }) {
                Ok(_) => Ok(()),
                Err(e) => Err(Box::new(e) as Box<dyn std::error::Error>),
            }
        } else {
            let mut client = get_o2m_client();
            match TOKIO_RUNTIME.block_on(client.o2m_set_confidentiality(request)) {
                Ok(_) => Ok(()),
                Err(e) => Err(Box::new(e) as Box<dyn std::error::Error>),
            }
        }
    }

    #[cfg(feature = "o2m")]
    pub fn set_integrity(
        resource: proto::primitives::Resource,
        integrity: u32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let request = tonic::Request::new(proto::messages::SetIntegrityRequest {
            resource: Some(resource),
            integrity,
        });

        if let Ok(handle) = Handle::try_current() {
            match block_in_place(move || {
                let mut client = get_o2m_client();
                handle.block_on(client.o2m_set_integrity(request))
            }) {
                Ok(_) => Ok(()),
                Err(e) => Err(Box::new(e) as Box<dyn std::error::Error>),
            }
        } else {
            let mut client = get_o2m_client();
            match TOKIO_RUNTIME.block_on(client.o2m_set_integrity(request)) {
                Ok(_) => Ok(()),
                Err(e) => Err(Box::new(e) as Box<dyn std::error::Error>),
            }
        }
    }

    #[cfg(feature = "o2m")]
    pub fn set_deleted(
        resource: proto::primitives::Resource,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let request = tonic::Request::new(proto::messages::SetDeletedRequest {
            resource: Some(resource),
        });

        if let Ok(handle) = Handle::try_current() {
            match block_in_place(move || {
                let mut client = get_o2m_client();
                handle.block_on(client.o2m_set_deleted(request))
            }) {
                Ok(_) => Ok(()),
                Err(e) => Err(Box::new(e) as Box<dyn std::error::Error>),
            }
        } else {
            let mut client = get_o2m_client();
            match TOKIO_RUNTIME.block_on(client.o2m_set_deleted(request)) {
                Ok(_) => Ok(()),
                Err(e) => Err(Box::new(e) as Box<dyn std::error::Error>),
            }
        }
    }

    #[cfg(feature = "o2m")]
    pub fn enforce_consent(
        resource: proto::primitives::Resource,
    ) -> Result<
        tonic::codec::Streaming<proto::messages::ConsentNotification>,
        Box<dyn std::error::Error>,
    > {
        let request = tonic::Request::new(proto::messages::EnforceConsentRequest {
            resource: Some(resource),
        });

        if let Ok(handle) = Handle::try_current() {
            match task::block_in_place(move || {
                let mut client = get_o2m_client();
                handle.block_on(client.o2m_enforce_consent(request))
            }) {
                Ok(response) => Ok(response.into_inner()),
                Err(e) => Err(Box::new(e) as Box<dyn std::error::Error>),
            }
        } else {
            let mut client = get_o2m_client();
            match TOKIO_RUNTIME.block_on(client.o2m_enforce_consent(request)) {
                Ok(response) => Ok(response.into_inner()),
                Err(e) => Err(Box::new(e) as Box<dyn std::error::Error>),
            }
        }
    }

    #[cfg(feature = "o2m")]
    pub fn set_consent_decision(
        source: proto::primitives::Resource,
        destination: proto::primitives::Destination,
        decision: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let request = tonic::Request::new(proto::messages::SetConsentDecisionRequest {
            source: Some(source),
            destination: Some(destination),
            decision,
        });

        if let Ok(handle) = Handle::try_current() {
            match block_in_place(move || {
                let mut client = get_o2m_client();
                handle.block_on(client.o2m_set_consent_decision(request))
            }) {
                Ok(_) => Ok(()),
                Err(e) => Err(Box::new(e) as Box<dyn std::error::Error>),
            }
        } else {
            let mut client = get_o2m_client();
            match TOKIO_RUNTIME.block_on(client.o2m_set_consent_decision(request)) {
                Ok(_) => Ok(()),
                Err(e) => Err(Box::new(e) as Box<dyn std::error::Error>),
            }
        }
    }

    #[cfg(feature = "o2m")]
    pub fn get_references(
        resource: proto::primitives::Resource,
    ) -> Result<Vec<proto::primitives::References>, Box<dyn std::error::Error>> {
        let request = tonic::Request::new(proto::messages::GetReferencesRequest {
            resource: Some(resource),
        });

        if let Ok(handle) = Handle::try_current() {
            match task::block_in_place(move || {
                let mut client = get_o2m_client();
                handle.block_on(client.o2m_get_references(request))
            }) {
                Ok(response) => Ok(response.into_inner().references),
                Err(e) => Err(Box::new(e) as Box<dyn std::error::Error>),
            }
        } else {
            let mut client = get_o2m_client();
            match TOKIO_RUNTIME.block_on(client.o2m_get_references(request)) {
                Ok(response) => Ok(response.into_inner().references),
                Err(e) => Err(Box::new(e) as Box<dyn std::error::Error>),
            }
        }
    }
}

pub use grpc::{io_report, io_request, local_enroll, remote_enroll};

#[cfg(feature = "o2m")]
pub use grpc::{
    enforce_consent, get_policies, get_references, set_confidentiality, set_consent_decision,
    set_deleted, set_integrity, set_policy,
};

pub use proto::primitives::Flow;

#[cfg(feature = "o2m")]
pub use proto::primitives::{
    Confidentiality, Destination, LocalizedResource, LocalizedResourceWithParent,
    MappedLocalizedPolicy, Policy, References, Resource,
};

#[cfg(feature = "o2m")]
pub use proto::messages::ConsentNotification;
