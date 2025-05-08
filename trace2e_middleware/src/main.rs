use tonic::transport::Server;
use tonic_reflection::server::Builder;
use trace2e_middleware::{
    identifier,
    m2m_service::{
        m2m::{m2m_server::M2mServer, M2M_DESCRIPTOR_SET},
        M2mService,
    },
    p2m_dbus::P2mDbus,
    p2m_service::{
        p2m::{p2m_server::P2mServer, P2M_DESCRIPTOR_SET},
        P2mService,
    },
    traceability::{spawn_traceability_server, TraceabilityClient},
    user_service::{
        user::{user_server::UserServer, USER_DESCRIPTOR_SET},
        UserService,
    },
};
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};
use zbus::connection;

#[cfg(not(tarpaulin_include))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fmt_layer = fmt::layer().with_target(false);
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("off"))
        .unwrap();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();

    let _ = identifier::MIDDLEWARE_ID.get_or_init(|| {
        rustix::system::uname()
            .nodename()
            .to_str()
            .unwrap()
            .to_string()
            .to_lowercase()
    });

    let traceability = TraceabilityClient::new(spawn_traceability_server());

    let p2m_dbus = P2mDbus::new(traceability.clone());
    let _conn = connection::Builder::session()?
        .name("org.trace2e.P2m")?
        .serve_at("/org/trace2e/P2m", p2m_dbus)?
        .build()
        .await?;

    let address = "[::]:8080".parse().unwrap();
    let p2m_service = P2mService::new(traceability.clone());
    let m2m_service = M2mService::new(traceability.clone());
    let user_service = UserService::new(traceability.clone());

    let reflection_service = Builder::configure()
        .register_encoded_file_descriptor_set(P2M_DESCRIPTOR_SET)
        .register_encoded_file_descriptor_set(M2M_DESCRIPTOR_SET)
        .register_encoded_file_descriptor_set(USER_DESCRIPTOR_SET)
        .build_v1()?;

    Server::builder()
        .add_service(P2mServer::new(p2m_service))
        .add_service(M2mServer::new(m2m_service))
        .add_service(UserServer::new(user_service))
        .add_service(reflection_service)
        .serve(address)
        .await?;

    Ok(())
}
