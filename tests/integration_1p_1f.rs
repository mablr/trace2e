use trace2e::p2m_service::{p2m::{p2m_client::P2mClient, p2m_server::P2mServer, Ack, Ct, Io, Grant}, P2mService};
use tonic::transport::Server;
use std::time::Duration;
use tokio::time::sleep;

async fn server_light() {
    let address = "[::1]:8080".parse().unwrap();
    let p2m_service = P2mService::new();

    Server::builder()
        .add_service(P2mServer::new(p2m_service))
        .serve(address)
        .await
        .unwrap()   
}

#[tokio::test]
async fn integration_1p_1f() -> Result<(), Box<dyn std::error::Error>> {
    tokio::spawn(server_light());
    // Give the server a moment to start
    sleep(Duration::from_secs(1)).await;

    // test
    let mut client = P2mClient::connect("http://[::1]:8080").await?;
    let request = tonic::Request::new(Ct { 
        process_id: 1,
        file_descriptor: 1,
        container: "std::fs::File".to_string(),
        resource_identifier: "/test-file".to_string(),
        input: true,
        output: false,
        remote: true
    });
    let result = client.ct_event(request).await?.into_inner();
    assert_eq!(result, Ack {});
    let request = tonic::Request::new(Io {
        process_id: 1,
        file_descriptor: 1,
        container: "std::fs::File".to_string(),
        method: "write".to_string()
    });
    let result = client.io_event(request).await?.into_inner();
    assert_eq!(result, Grant {file_descriptor: 1});
    Ok(())       
}
