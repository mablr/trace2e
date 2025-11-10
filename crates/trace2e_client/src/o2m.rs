use once_cell::{sync::Lazy, unsync::OnceCell};
use tokio::{
    runtime::Handle,
    task::{self, block_in_place},
};
use tonic::transport::Channel;

use crate::proto;

static TOKIO_RUNTIME: Lazy<tokio::runtime::Runtime> =
    Lazy::new(|| tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap());

// The URL for the gRPC service
const GRPC_URL: &str = "http://[::1]:50051";

static O2M_CLIENT: Lazy<proto::o2m_client::O2mClient<Channel>> = Lazy::new(|| {
    let rt = &*TOKIO_RUNTIME;
    rt.block_on(proto::o2m_client::O2mClient::connect(GRPC_URL)).unwrap()
});

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

pub fn set_deleted(
    resource: proto::primitives::Resource,
) -> Result<(), Box<dyn std::error::Error>> {
    let request =
        tonic::Request::new(proto::messages::SetDeletedRequest { resource: Some(resource) });

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

pub fn enforce_consent(
    resource: proto::primitives::Resource,
) -> Result<tonic::codec::Streaming<proto::messages::ConsentNotification>, Box<dyn std::error::Error>>
{
    let request =
        tonic::Request::new(proto::messages::EnforceConsentRequest { resource: Some(resource) });

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

pub fn get_references(
    resource: proto::primitives::Resource,
) -> Result<Vec<proto::primitives::References>, Box<dyn std::error::Error>> {
    let request =
        tonic::Request::new(proto::messages::GetReferencesRequest { resource: Some(resource) });

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
