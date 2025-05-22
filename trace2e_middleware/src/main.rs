use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

use tonic::transport::Server;
use tonic_reflection::server::Builder;
use trace2e_middleware::{
    Trace2eService,
    grpc_proto::{MIDDLEWARE_DESCRIPTOR_SET, trace2e_server::Trace2eServer},
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

    let address = "[::]:8080".parse().unwrap();
    let reflection_service = Builder::configure()
        .register_encoded_file_descriptor_set(MIDDLEWARE_DESCRIPTOR_SET)
        .build_v1()?;

    Server::builder()
        .add_service(Trace2eServer::new(Trace2eService::default()))
        .add_service(reflection_service)
        .serve(address)
        .await?;

    Ok(())
}
