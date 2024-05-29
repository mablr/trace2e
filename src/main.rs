use trace2e::p2m::{p2m_server::P2mServer, FILE_DESCRIPTOR_SET};
use tonic::transport::Server;
use tonic_reflection::server::Builder;
mod p2m_service;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = "[::1]:8080".parse().unwrap();
    let p2m_service = p2m_service::P2mService::new();

    let reflection_service = Builder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .build()?;

    Server::builder()
        .add_service(P2mServer::new(p2m_service))
        .add_service(reflection_service)
        .serve(address)
        .await?;

    Ok(())       
}
