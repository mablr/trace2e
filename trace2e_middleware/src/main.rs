use tonic::transport::Server;
use tonic_reflection::server::Builder;
use tower::{ServiceBuilder, layer::layer_fn};
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

use trace2e_middleware::{
    Trace2eGrpcService,
    grpc_proto::{MIDDLEWARE_DESCRIPTOR_SET, trace2e_server::Trace2eServer},
    traceability::{
        layers::{
            provenance::ProvenanceService,
            sequencer::{SequencerService, WaitingQueueService},
        },
        p2m::P2mApiService,
    },
};

#[cfg(not(tarpaulin_include))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use trace2e_middleware::traceability::layers::compliance::ComplianceService;

    let fmt_layer = fmt::layer().with_target(false);
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("off"))
        .unwrap();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();

    let sequencer = ServiceBuilder::new()
        .layer(layer_fn(|inner| WaitingQueueService::new(inner, None)))
        .service(SequencerService::default());
    let provenance = ServiceBuilder::new().service(ProvenanceService::default());
    let compliance = ServiceBuilder::new().service(ComplianceService::default());

    let p2m_service =
        ServiceBuilder::new().service(P2mApiService::new(sequencer, provenance, compliance));

    let address = "[::]:8080".parse().unwrap();
    let reflection_service = Builder::configure()
        .register_encoded_file_descriptor_set(MIDDLEWARE_DESCRIPTOR_SET)
        .build_v1()?;

    Server::builder()
        .add_service(Trace2eServer::new(Trace2eGrpcService::new(p2m_service)))
        .add_service(reflection_service)
        .serve(address)
        .await?;

    Ok(())
}
