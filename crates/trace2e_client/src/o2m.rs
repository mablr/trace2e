use once_cell::{sync::Lazy, unsync::OnceCell};
use tokio::{
    runtime::Handle,
    task::{self, block_in_place},
};
use tonic::transport::Channel;
use trace2e_core::traceability::{infrastructure::naming, services::consent};
use trace2e_core::transport::grpc::proto;

static TOKIO_RUNTIME: Lazy<tokio::runtime::Runtime> =
    Lazy::new(|| tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap());

// Get the gRPC URL from environment variable or use default
fn get_grpc_url() -> String {
    std::env::var("TRACE2E_MIDDLEWARE_URL").unwrap_or_else(|_| "http://[::1]:50051".to_string())
}

static O2M_CLIENT: Lazy<proto::o2m_client::O2mClient<Channel>> = Lazy::new(|| {
    let rt = &*TOKIO_RUNTIME;
    rt.block_on(proto::o2m_client::O2mClient::connect(get_grpc_url())).unwrap()
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
                        .block_on(proto::o2m_client::O2mClient::connect(get_grpc_url()))
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
    resources: Vec<naming::Resource>,
) -> Result<Vec<proto::primitives::MappedLocalizedPolicy>, Box<dyn std::error::Error>> {
    let proto_resources: Vec<proto::primitives::Resource> =
        resources.into_iter().map(|r| r.into()).collect();
    let request =
        tonic::Request::new(proto::messages::GetPoliciesRequest { resources: proto_resources });

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
    resource: naming::Resource,
    policy: proto::primitives::Policy,
) -> Result<(), Box<dyn std::error::Error>> {
    let proto_resource: proto::primitives::Resource = resource.into();
    let request = tonic::Request::new(proto::messages::SetPolicyRequest {
        resource: Some(proto_resource),
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
    resource: naming::Resource,
    confidentiality: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    let proto_resource: proto::primitives::Resource = resource.into();
    let request = tonic::Request::new(proto::messages::SetConfidentialityRequest {
        resource: Some(proto_resource),
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
    resource: naming::Resource,
    integrity: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let proto_resource: proto::primitives::Resource = resource.into();
    let request = tonic::Request::new(proto::messages::SetIntegrityRequest {
        resource: Some(proto_resource),
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

pub fn set_deleted(resource: naming::Resource) -> Result<(), Box<dyn std::error::Error>> {
    let proto_resource: proto::primitives::Resource = resource.into();
    let request =
        tonic::Request::new(proto::messages::SetDeletedRequest { resource: Some(proto_resource) });

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
    resource: naming::Resource,
) -> Result<tonic::codec::Streaming<proto::messages::ConsentNotification>, Box<dyn std::error::Error>>
{
    let proto_resource: proto::primitives::Resource = resource.into();
    let request = tonic::Request::new(proto::messages::EnforceConsentRequest {
        resource: Some(proto_resource),
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

pub fn set_consent_decision(
    source: naming::Resource,
    destination: consent::Destination,
    decision: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let proto_source: proto::primitives::Resource = source.into();
    let proto_destination: proto::primitives::Destination = destination.into();
    let request = tonic::Request::new(proto::messages::SetConsentDecisionRequest {
        source: Some(proto_source),
        destination: Some(proto_destination),
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
    resource: naming::Resource,
) -> Result<Vec<proto::primitives::References>, Box<dyn std::error::Error>> {
    let proto_resource: proto::primitives::Resource = resource.into();
    let request = tonic::Request::new(proto::messages::GetReferencesRequest {
        resource: Some(proto_resource),
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
