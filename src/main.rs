use std::sync::Arc;

use tokio::sync::{mpsc, Mutex};
use tonic::transport::Server;
use tonic_reflection::server::Builder;
use trace2e::{
    containers::{congestion_manager, ContainersManager, QueuingHandler},
    p2m_service::{
        p2m::{p2m_server::P2mServer, FILE_DESCRIPTOR_SET},
        P2mService,
    },
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let containers_manager = ContainersManager::default();

    let (sender, receiver) = mpsc::channel(32);
    let queuing_handler = Arc::new(Mutex::new(QueuingHandler::default()));

    tokio::spawn(congestion_manager(receiver, queuing_handler));

    let address = "[::1]:8080".parse().unwrap();
    let p2m_service = P2mService::new(containers_manager, sender);

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
