use tokio::sync::mpsc;
use tonic::transport::Server;
use tonic_reflection::server::Builder;
use trace2e::{
    p2m_service::{
        p2m::{p2m_server::P2mServer, FILE_DESCRIPTOR_SET},
        P2mService,
    },
    provenance::provenance_layer,
};

#[cfg(not(tarpaulin_include))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (sender, receiver) = mpsc::channel(32);

    tokio::spawn(provenance_layer(receiver));

    let address = "[::]:8080".parse().unwrap();
    let p2m_service = P2mService::new(sender);

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
