use trace2e::p2m_service::p2m::{p2m_client::P2mClient, Ack, Ct, Io, Grant};

#[tokio::test]
async fn integration_mp_1f_write() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = P2mClient::connect("http://[::1]:8080").await?;

    // CT declaration
    let file_creation = tonic::Request::new(Ct { 
        process_id: 10000,
        file_descriptor: 3,
        container: "std::fs::File".to_string(),
        resource_identifier: "bar.txt".to_string(),
        input: true,
        output: false,
        remote: false
    });
    let result_file_creation = client.ct_event(file_creation).await?.into_inner();
    assert_eq!(result_file_creation, Ack {});

    // Write event
    let write_request = tonic::Request::new(Io {
        process_id: 10000,
        file_descriptor: 3,
        container: "std::fs::File".to_string(),
        method: "write_fmt".to_string()
    });
    let result_write_request = client.io_event(write_request).await?.into_inner();
    assert_eq!(result_write_request, Grant {file_descriptor: 3});

    // Write done
    let write_done = tonic::Request::new(Io {
        process_id: 10000,
        file_descriptor: 3,
        container: "std::fs::File".to_string(),
        method: "write_fmt".to_string()
    });
    let result_write_done = client.done_io_event(write_done).await?.into_inner();
    assert_eq!(result_write_done, Ack {});
    Ok(())
}

#[tokio::test]
async fn integration_mp_1f_read() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = P2mClient::connect("http://[::1]:8080").await?;

    // CT declaration
    let file_creation = tonic::Request::new(Ct { 
        process_id: 10001,
        file_descriptor: 3,
        container: "std::fs::File".to_string(),
        resource_identifier: "bar.txt".to_string(),
        input: false,
        output: true,
        remote: false
    });
    let result_file_creation = client.ct_event(file_creation).await?.into_inner();
    assert_eq!(result_file_creation, Ack {});

    // Read event
    let read_request = tonic::Request::new(Io {
        process_id: 10001,
        file_descriptor: 3,
        container: "std::fs::File".to_string(),
        method: "read".to_string()
    });
    let result_read_request = client.io_event(read_request).await?.into_inner();
    assert_eq!(result_read_request, Grant {file_descriptor: 3});

    // Read done
    let read_done = tonic::Request::new(Io {
        process_id: 10001,
        file_descriptor: 3,
        container: "std::fs::File".to_string(),
        method: "read".to_string()
    });
    let result_read_done = client.done_io_event(read_done).await?.into_inner();
    assert_eq!(result_read_done, Ack {});
    Ok(())
}
