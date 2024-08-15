use std::process::Command;

use trace2e::p2m_service::p2m::{p2m_client::P2mClient, Ack, Flow, IoInfo, IoResult, LocalCt};

#[tokio::test]
async fn integration_1p_1f_write() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = P2mClient::connect("http://[::1]:8080").await?;
    let mut process = Command::new("tail").arg("-f").arg("/dev/null").spawn()?;
    // CT declaration
    let file_creation = tonic::Request::new(LocalCt {
        process_id: process.id(),
        file_descriptor: 3,
        path: "bar.txt".to_string(),
    });
    let result_file_creation = client.local_enroll(file_creation).await?.into_inner();
    assert_eq!(result_file_creation, Ack {});

    // Write event
    let write_request = tonic::Request::new(IoInfo {
        process_id: process.id(),
        file_descriptor: 3,
        flow: Flow::Input.into(),
    });
    let result_write_request = client.io_request(write_request).await?;
    let grant_id = result_write_request.into_inner().id;

    // Write done
    let write_done = tonic::Request::new(IoResult {
        process_id: process.id(),
        file_descriptor: 3,
        grant_id,
        result: true,
    });
    let result_write_done = client.io_report(write_done).await?.into_inner();
    assert_eq!(result_write_done, Ack {});

    process.kill()?;
    Ok(())
}

#[tokio::test]
async fn integration_1p_1f_read() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = P2mClient::connect("http://[::1]:8080").await?;
    let mut process = Command::new("tail").arg("-f").arg("/dev/null").spawn()?;

    // CT declaration
    let file_creation = tonic::Request::new(LocalCt {
        process_id: process.id(),
        file_descriptor: 3,
        path: "foo.txt".to_string(),
    });
    let result_file_creation = client.local_enroll(file_creation).await?.into_inner();
    assert_eq!(result_file_creation, Ack {});

    // Read event
    let read_request = tonic::Request::new(IoInfo {
        process_id: process.id(),
        file_descriptor: 3,
        flow: Flow::Output.into(),
    });
    let result_read_request = client.io_request(read_request).await?;
    let grant_id = result_read_request.into_inner().id;

    // Read done
    let read_done = tonic::Request::new(IoResult {
        process_id: process.id(),
        file_descriptor: 3,
        grant_id,
        result: true,
    });
    let result_read_done = client.io_report(read_done).await?.into_inner();
    assert_eq!(result_read_done, Ack {});

    process.kill()?;
    Ok(())
}

#[tokio::test]
async fn integration_1p_1f_complex() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = P2mClient::connect("http://[::1]:8080").await?;
    let mut process = Command::new("tail").arg("-f").arg("/dev/null").spawn()?;

    // CT declaration
    let file_creation = tonic::Request::new(LocalCt {
        process_id: process.id(),
        file_descriptor: 3,
        path: "foobar.txt".to_string(),
    });
    let result_file_creation = client.local_enroll(file_creation).await?.into_inner();
    assert_eq!(result_file_creation, Ack {});

    // Write event
    let write_request1 = tonic::Request::new(IoInfo {
        process_id: process.id(),
        file_descriptor: 3,
        flow: Flow::Input.into(),
    });
    let result_write_request1 = client.io_request(write_request1).await?;
    let grant_id = result_write_request1.into_inner().id;

    // Write done
    let write_done1 = tonic::Request::new(IoResult {
        process_id: process.id(),
        file_descriptor: 3,
        grant_id,
        result: true,
    });
    let result_write_done1 = client.io_report(write_done1).await?.into_inner();
    assert_eq!(result_write_done1, Ack {});

    // Write event
    let write_request2 = tonic::Request::new(IoInfo {
        process_id: process.id(),
        file_descriptor: 3,
        flow: Flow::Input.into(),
    });
    let result_write_request2 = client.io_request(write_request2).await?;
    let grant_id = result_write_request2.into_inner().id;

    // Write done
    let write_done2 = tonic::Request::new(IoResult {
        process_id: process.id(),
        file_descriptor: 3,
        grant_id,
        result: true,
    });
    let result_write_done2 = client.io_report(write_done2).await?.into_inner();
    assert_eq!(result_write_done2, Ack {});

    // CT declaration
    let file_reopen = tonic::Request::new(LocalCt {
        process_id: process.id(),
        file_descriptor: 4,
        path: "foobar.txt".to_string(),
    });
    let result_file_reopen = client.local_enroll(file_reopen).await?.into_inner();
    assert_eq!(result_file_reopen, Ack {});

    // Seek event
    let seek_request = tonic::Request::new(IoInfo {
        process_id: process.id(),
        file_descriptor: 4,
        flow: Flow::None.into(),
    });
    let result_seek_request = client.io_request(seek_request).await.unwrap_err();
    assert_eq!(result_seek_request.message(), "Unsupported Flow type");

    // seek done
    let seek_done = tonic::Request::new(IoResult {
        process_id: process.id(),
        file_descriptor: 4,
        grant_id: 0,
        result: true,
    });
    let result_seek_done = client.io_report(seek_done).await.unwrap_err();
    assert_eq!(result_seek_done.message(), "Flow 0 does not exist");

    // Read event
    let read_request = tonic::Request::new(IoInfo {
        process_id: process.id(),
        file_descriptor: 4,
        flow: Flow::Output.into(),
    });
    let result_read_request = client.io_request(read_request).await?;
    let grant_id = result_read_request.into_inner().id;

    // Read done
    let read_done = tonic::Request::new(IoResult {
        process_id: process.id(),
        file_descriptor: 4,
        grant_id,
        result: true,
    });
    let result_read_done = client.io_report(read_done).await?.into_inner();
    assert_eq!(result_read_done, Ack {});

    process.kill()?;
    Ok(())
}
