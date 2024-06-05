use trace2e::p2m_service::p2m::{p2m_client::P2mClient, Ack, Ct, Io, Grant};

#[tokio::test]
async fn integration_1p_1f_write() -> Result<(), Box<dyn std::error::Error>> {
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
async fn integration_1p_1f_read() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = P2mClient::connect("http://[::1]:8080").await?;

    // CT declaration
    let file_creation = tonic::Request::new(Ct { 
        process_id: 10000,
        file_descriptor: 3,
        container: "std::fs::File".to_string(),
        resource_identifier: "foo.txt".to_string(),
        input: false,
        output: true,
        remote: false
    });
    let result_file_creation = client.ct_event(file_creation).await?.into_inner();
    assert_eq!(result_file_creation, Ack {});

    // Read event
    let read_request = tonic::Request::new(Io {
        process_id: 10000,
        file_descriptor: 3,
        container: "std::fs::File".to_string(),
        method: "read".to_string()
    });
    let result_read_request = client.io_event(read_request).await?.into_inner();
    assert_eq!(result_read_request, Grant {file_descriptor: 3});

    // Read done
    let read_done = tonic::Request::new(Io {
        process_id: 10000,
        file_descriptor: 3,
        container: "std::fs::File".to_string(),
        method: "read".to_string()
    });
    let result_read_done = client.done_io_event(read_done).await?.into_inner();
    assert_eq!(result_read_done, Ack {});
    Ok(())
}

#[tokio::test]
async fn integration_1p_1f_complex() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = P2mClient::connect("http://[::1]:8080").await?;

    // CT declaration
    let file_creation = tonic::Request::new(Ct { 
        process_id: 10000,
        file_descriptor: 3,
        container: "std::fs::File".to_string(),
        resource_identifier: "foobar.txt".to_string(),
        input: true,
        output: false,
        remote: false
    });
    let result_file_creation = client.ct_event(file_creation).await?.into_inner();
    assert_eq!(result_file_creation, Ack {});

    // Write event
    let write_request1 = tonic::Request::new(Io {
        process_id: 10000,
        file_descriptor: 3,
        container: "std::fs::File".to_string(),
        method: "write_fmt".to_string()
    });
    let result_write_request1 = client.io_event(write_request1).await?.into_inner();
    assert_eq!(result_write_request1, Grant {file_descriptor: 3});

    // Write done
    let write_done1 = tonic::Request::new(Io {
        process_id: 10000,
        file_descriptor: 3,
        container: "std::fs::File".to_string(),
        method: "write_fmt".to_string()
    });
    let result_write_done1 = client.done_io_event(write_done1).await?.into_inner();
    assert_eq!(result_write_done1, Ack {});

    // Write event
    let write_request2 = tonic::Request::new(Io {
        process_id: 10000,
        file_descriptor: 3,
        container: "std::fs::File".to_string(),
        method: "write_fmt".to_string()
    });
    let result_write_request2 = client.io_event(write_request2).await?.into_inner();
    assert_eq!(result_write_request2, Grant {file_descriptor: 3});

    // Write done
    let write_done2 = tonic::Request::new(Io {
        process_id: 10000,
        file_descriptor: 3,
        container: "std::fs::File".to_string(),
        method: "write_fmt".to_string()
    });
    let result_write_done2 = client.done_io_event(write_done2).await?.into_inner();
    assert_eq!(result_write_done2, Ack {});

    // CT declaration
    let file_reopen = tonic::Request::new(Ct { 
        process_id: 10000,
        file_descriptor: 4,
        container: "std::fs::File".to_string(),
        resource_identifier: "foobar.txt".to_string(),
        input: false,
        output: true,
        remote: false
    });
    let result_file_reopen = client.ct_event(file_reopen).await?.into_inner();
    assert_eq!(result_file_reopen, Ack {});

    // Seek event
    let seek_request = tonic::Request::new(Io {
        process_id: 10000,
        file_descriptor: 4,
        container: "std::fs::File".to_string(),
        method: "seek".to_string()
    });
    let result_seek_request = client.io_event(seek_request).await?.into_inner();
    assert_eq!(result_seek_request, Grant {file_descriptor: 4});

    // seek done
    let seek_done = tonic::Request::new(Io {
        process_id: 10000,
        file_descriptor: 4,
        container: "std::fs::File".to_string(),
        method: "seek".to_string()
    });
    let result_seek_done = client.done_io_event(seek_done).await?.into_inner();
    assert_eq!(result_seek_done, Ack {});

    // Read event
    let read_request = tonic::Request::new(Io {
        process_id: 10000,
        file_descriptor: 4,
        container: "std::fs::File".to_string(),
        method: "read_to_string".to_string()
    });
    let result_read_request = client.io_event(read_request).await?.into_inner();
    assert_eq!(result_read_request, Grant {file_descriptor: 4});

    // Read done
    let read_done = tonic::Request::new(Io {
        process_id: 10000,
        file_descriptor: 4,
        container: "std::fs::File".to_string(),
        method: "read_to_string".to_string()
    });
    let result_read_done = client.done_io_event(read_done).await?.into_inner();
    assert_eq!(result_read_done, Ack {});
    Ok(())
}