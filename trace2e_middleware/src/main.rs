use tonic::transport::Server;
use tonic_reflection::server::Builder;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

use trace2e_middleware::{
    traceability::init_middleware,
    transport::grpc::{
        DEFAULT_GRPC_PORT, M2mGrpc, Trace2eRouter,
        proto::{MIDDLEWARE_DESCRIPTOR_SET, trace2e_grpc_server::Trace2eGrpcServer},
    },
};

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

    let (m2m_service, p2m_service) = init_middleware(None, M2mGrpc::default());

    let address = format!("[::]:{DEFAULT_GRPC_PORT}").parse().unwrap();
    let reflection_service = Builder::configure()
        .register_encoded_file_descriptor_set(MIDDLEWARE_DESCRIPTOR_SET)
        .build_v1()?;

    Server::builder()
        .add_service(Trace2eGrpcServer::new(Trace2eRouter::new(
            p2m_service,
            m2m_service,
        )))
        .add_service(reflection_service)
        .serve(address)
        .await?;

    Ok(())
}
