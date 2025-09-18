use clap::Parser;
use tonic::transport::Server;
use tonic_reflection::server::Builder;
use trace2e_core::{
    traceability::{init_middleware, init_middleware_with_validation},
    transport::grpc::{
        DEFAULT_GRPC_PORT, M2mGrpc, Trace2eRouter,
        proto::{MIDDLEWARE_DESCRIPTOR_SET, trace2e_grpc_server::Trace2eGrpcServer},
    },
};

#[derive(Parser, Debug)]
#[command(name = "trace2e_middleware")]
#[command(about = "Trace2e middleware server")]
struct Trace2eMiddlewareArgs {
    /// Server address to bind to
    #[arg(short, long, default_value = "[::1]")]
    address: String,

    /// Server port to bind to
    #[arg(short, long, default_value_t = DEFAULT_GRPC_PORT)]
    port: u16,

    /// Enable gRPC reflection
    #[arg(short, long, default_value_t = false)]
    reflection: bool,

    /// Disable resource validation for P2M requests
    #[arg(long, default_value_t = false)]
    disable_resource_validation: bool,
}

#[cfg(not(tarpaulin_include))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "trace2e_tracing")]
    trace2e_core::trace2e_tracing::init();

    let args = Trace2eMiddlewareArgs::parse();

    let address = format!("{}:{}", args.address, args.port).parse().unwrap();

    let mut server_builder = if args.disable_resource_validation {
        let (m2m_service, p2m_service, _) = init_middleware(args.address.clone(), None, M2mGrpc::default());
        Server::builder()
            .add_service(Trace2eGrpcServer::new(Trace2eRouter::new(p2m_service, m2m_service)))
    } else {
        let (m2m_service, p2m_service, _) = init_middleware_with_validation(args.address.clone(), None, M2mGrpc::default());
        Server::builder()
            .add_service(Trace2eGrpcServer::new(Trace2eRouter::new(p2m_service, m2m_service)))
    };

    if args.reflection {
        let reflection_service = Builder::configure()
            .register_encoded_file_descriptor_set(MIDDLEWARE_DESCRIPTOR_SET)
            .build_v1()?;
        server_builder = server_builder.add_service(reflection_service);
    }

    server_builder.serve(address).await?;

    Ok(())
}
