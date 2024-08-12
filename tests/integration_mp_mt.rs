use trace2e::p2m_service::p2m::{p2m_client::P2mClient, Ack, Flow, IoInfo, IoResult, LocalCt};

#[tokio::test]
async fn integration_mp_mt_1f_write1() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = P2mClient::connect("http://[::1]:8080").await?;

    // CT declaration
    let file_creation = tonic::Request::new(LocalCt {
        process_id: 10000,
        file_descriptor: 3,
        path: "bar.txt".to_string(),
    });
    let result_file_creation = client.local_enroll(file_creation).await?.into_inner();
    assert_eq!(result_file_creation, Ack {});

    // Write event
    let write_request = tonic::Request::new(IoInfo {
        process_id: 10000,
        file_descriptor: 3,
        flow: Flow::Output.into(),
    });
    let result_write_request = client.io_request(write_request).await?;
    let grant_id = result_write_request.into_inner().id;

    // Write done
    let write_done = tonic::Request::new(IoResult {
        process_id: 10000,
        file_descriptor: 3,
        grant_id,
        result: true,
    });
    let result_write_done = client.io_report(write_done).await?.into_inner();
    assert_eq!(result_write_done, Ack {});
    Ok(())
}

#[tokio::test]
async fn integration_mp_mt_1f_write2() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = P2mClient::connect("http://[::1]:8080").await?;

    // CT declaration
    let file_creation = tonic::Request::new(LocalCt {
        process_id: 10000,
        file_descriptor: 4,
        path: "bar1.txt".to_string(),
    });
    let result_file_creation = client.local_enroll(file_creation).await?.into_inner();
    assert_eq!(result_file_creation, Ack {});

    // Write event
    let write_request = tonic::Request::new(IoInfo {
        process_id: 10000,
        file_descriptor: 4,
        flow: Flow::Output.into(),
    });
    let result_write_request = client.io_request(write_request).await?;
    let grant_id = result_write_request.into_inner().id;

    // Write done
    let write_done = tonic::Request::new(IoResult {
        process_id: 10000,
        file_descriptor: 4,
        grant_id,
        result: true,
    });
    let result_write_done = client.io_report(write_done).await?.into_inner();
    assert_eq!(result_write_done, Ack {});
    Ok(())
}

#[tokio::test]
async fn integration_mp_mt_1f_write3() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = P2mClient::connect("http://[::1]:8080").await?;

    // CT declaration
    let file_creation = tonic::Request::new(LocalCt {
        process_id: 10000,
        file_descriptor: 5,
        path: "bar2.txt".to_string(),
    });
    let result_file_creation = client.local_enroll(file_creation).await?.into_inner();
    assert_eq!(result_file_creation, Ack {});

    // Write event
    let write_request = tonic::Request::new(IoInfo {
        process_id: 10000,
        file_descriptor: 5,
        flow: Flow::Output.into(),
    });
    let result_write_request = client.io_request(write_request).await?;
    let grant_id = result_write_request.into_inner().id;

    // Write done
    let write_done = tonic::Request::new(IoResult {
        process_id: 10000,
        file_descriptor: 5,
        grant_id,
        result: true,
    });
    let result_write_done = client.io_report(write_done).await?.into_inner();
    assert_eq!(result_write_done, Ack {});
    Ok(())
}

#[tokio::test]
async fn integration_mp_mt_1f_read1() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = P2mClient::connect("http://[::1]:8080").await?;

    // CT declaration
    let file_creation = tonic::Request::new(LocalCt {
        process_id: 10000,
        file_descriptor: 6,
        path: "bar3.txt".to_string(),
    });
    let result_file_creation = client.local_enroll(file_creation).await?.into_inner();
    assert_eq!(result_file_creation, Ack {});

    // Read event
    let read_request = tonic::Request::new(IoInfo {
        process_id: 10000,
        file_descriptor: 6,
        flow: Flow::Input.into(),
    });
    let result_read_request = client.io_request(read_request).await?;
    let grant_id = result_read_request.into_inner().id;

    // Read done
    let read_done = tonic::Request::new(IoResult {
        process_id: 10000,
        file_descriptor: 6,
        grant_id,
        result: true,
    });
    let result_read_done = client.io_report(read_done).await?.into_inner();
    assert_eq!(result_read_done, Ack {});
    Ok(())
}

#[tokio::test]
async fn integration_mp_mt_1f_read2() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = P2mClient::connect("http://[::1]:8080").await?;

    // CT declaration
    let file_creation = tonic::Request::new(LocalCt {
        process_id: 10000,
        file_descriptor: 7,
        path: "bar4.txt".to_string(),
    });
    let result_file_creation = client.local_enroll(file_creation).await?.into_inner();
    assert_eq!(result_file_creation, Ack {});

    // Read event
    let read_request = tonic::Request::new(IoInfo {
        process_id: 10000,
        file_descriptor: 7,
        flow: Flow::Input.into(),
    });
    let result_read_request = client.io_request(read_request).await?;
    let grant_id = result_read_request.into_inner().id;

    // Read done
    let read_done = tonic::Request::new(IoResult {
        process_id: 10000,
        file_descriptor: 7,
        grant_id,
        result: true,
    });
    let result_read_done = client.io_report(read_done).await?.into_inner();
    assert_eq!(result_read_done, Ack {});
    Ok(())
}

#[tokio::test]
async fn integration_mp_mt_1f_read3() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = P2mClient::connect("http://[::1]:8080").await?;

    // CT declaration
    let file_creation = tonic::Request::new(LocalCt {
        process_id: 10000,
        file_descriptor: 8,
        path: "bar5.txt".to_string(),
    });
    let result_file_creation = client.local_enroll(file_creation).await?.into_inner();
    assert_eq!(result_file_creation, Ack {});

    // Read event
    let read_request = tonic::Request::new(IoInfo {
        process_id: 10000,
        file_descriptor: 8,
        flow: Flow::Input.into(),
    });
    let result_read_request = client.io_request(read_request).await?;
    let grant_id = result_read_request.into_inner().id;

    // Read done
    let read_done = tonic::Request::new(IoResult {
        process_id: 10000,
        file_descriptor: 8,
        grant_id,
        result: true,
    });
    let result_read_done = client.io_report(read_done).await?.into_inner();
    assert_eq!(result_read_done, Ack {});
    Ok(())
}
