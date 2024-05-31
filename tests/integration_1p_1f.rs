use trace2e::p2m_service::p2m::{p2m_client::P2mClient, Ack, Ct, Io, Grant};

#[tokio::test]
async fn integration_1p_1f() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = P2mClient::connect("http://[::1]:8080").await?;

    // CT declaration
    let request = tonic::Request::new(Ct { 
        process_id: 10000,
        file_descriptor: 3,
        container: "std::fs::File".to_string(),
        resource_identifier: "/test-file".to_string(),
        input: true,
        output: false,
        remote: true
    });
    let result = client.ct_event(request).await?.into_inner();
    assert_eq!(result, Ack {});

    // Write event
    let request = tonic::Request::new(Io {
        process_id: 10000,
        file_descriptor: 3,
        container: "std::fs::File".to_string(),
        method: "write".to_string()
    });
    let result = client.io_event(request).await?.into_inner();
    assert_eq!(result, Grant {file_descriptor: 3});

    // Write done
    let request = tonic::Request::new(Io {
        process_id: 10000,
        file_descriptor: 3,
        container: "std::fs::File".to_string(),
        method: "write".to_string()
    });
    let result = client.done_io_event(request).await?.into_inner();
    assert_eq!(result, Ack {});
    Ok(())
}
