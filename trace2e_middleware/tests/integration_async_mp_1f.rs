use std::process::Command;

use trace2e_middleware::grpc_proto::{
    trace2e_client::Trace2eClient, Ack, Flow, IoInfo, IoResult, LocalCt,
};

#[tokio::test]
async fn integration_async_mp_1f_write1() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = Trace2eClient::connect("http://[::1]:8080").await?;
    let mut process = Command::new("tail").arg("-f").arg("/dev/null").spawn()?;
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    // CT declaration
    let file_creation = tonic::Request::new(LocalCt {
        process_id: process.id(),
        file_descriptor: 3,
        path: "bar.txt".to_string(),
    });
    let result_file_creation = client.p2m_local_enroll(file_creation).await?.into_inner();
    assert_eq!(result_file_creation, Ack {});

    // Write event
    let write_request = tonic::Request::new(IoInfo {
        process_id: process.id(),
        file_descriptor: 3,
        flow: Flow::Output.into(),
    });
    let result_write_request = client.p2m_io_request(write_request).await?;
    let grant_id = result_write_request.into_inner().id;

    // Write done
    let write_done = tonic::Request::new(IoResult {
        process_id: process.id(),
        file_descriptor: 3,
        grant_id,
        result: true,
    });
    let result_write_done = client.p2m_io_report(write_done).await?.into_inner();
    assert_eq!(result_write_done, Ack {});

    process.kill()?;
    Ok(())
}

#[tokio::test]
async fn integration_async_mp_1f_write2() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = Trace2eClient::connect("http://[::1]:8080").await?;
    let mut process = Command::new("tail").arg("-f").arg("/dev/null").spawn()?;
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    // CT declaration
    let file_creation = tonic::Request::new(LocalCt {
        process_id: process.id(),
        file_descriptor: 3,
        path: "bar.txt".to_string(),
    });
    let result_file_creation = client.p2m_local_enroll(file_creation).await?.into_inner();
    assert_eq!(result_file_creation, Ack {});

    // Write event
    let write_request = tonic::Request::new(IoInfo {
        process_id: process.id(),
        file_descriptor: 3,
        flow: Flow::Output.into(),
    });
    let result_write_request = client.p2m_io_request(write_request).await?;
    let grant_id = result_write_request.into_inner().id;

    // Write done
    let write_done = tonic::Request::new(IoResult {
        process_id: process.id(),
        file_descriptor: 3,
        grant_id,
        result: true,
    });
    let result_write_done = client.p2m_io_report(write_done).await?.into_inner();
    assert_eq!(result_write_done, Ack {});

    process.kill()?;
    Ok(())
}

#[tokio::test]
async fn integration_async_mp_1f_write3() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = Trace2eClient::connect("http://[::1]:8080").await?;
    let mut process = Command::new("tail").arg("-f").arg("/dev/null").spawn()?;
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    // CT declaration
    let file_creation = tonic::Request::new(LocalCt {
        process_id: process.id(),
        file_descriptor: 3,
        path: "bar.txt".to_string(),
    });
    let result_file_creation = client.p2m_local_enroll(file_creation).await?.into_inner();
    assert_eq!(result_file_creation, Ack {});

    // Write event
    let write_request = tonic::Request::new(IoInfo {
        process_id: process.id(),
        file_descriptor: 3,
        flow: Flow::Output.into(),
    });
    let result_write_request = client.p2m_io_request(write_request).await?;
    let grant_id = result_write_request.into_inner().id;

    // Write done
    let write_done = tonic::Request::new(IoResult {
        process_id: process.id(),
        file_descriptor: 3,
        grant_id,
        result: true,
    });
    let result_write_done = client.p2m_io_report(write_done).await?.into_inner();
    assert_eq!(result_write_done, Ack {});

    process.kill()?;
    Ok(())
}

#[tokio::test]
async fn integration_async_mp_1f_read1() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = Trace2eClient::connect("http://[::1]:8080").await?;
    let mut process = Command::new("tail").arg("-f").arg("/dev/null").spawn()?;
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    // CT declaration
    let file_creation = tonic::Request::new(LocalCt {
        process_id: process.id(),
        file_descriptor: 3,
        path: "bar.txt".to_string(),
    });
    let result_file_creation = client.p2m_local_enroll(file_creation).await?.into_inner();
    assert_eq!(result_file_creation, Ack {});

    // Read event
    let read_request = tonic::Request::new(IoInfo {
        process_id: process.id(),
        file_descriptor: 3,
        flow: Flow::Input.into(),
    });
    let result_read_request = client.p2m_io_request(read_request).await?;
    let grant_id = result_read_request.into_inner().id;

    // Read done
    let read_done = tonic::Request::new(IoResult {
        process_id: process.id(),
        file_descriptor: 3,
        grant_id,
        result: true,
    });
    let result_read_done = client.p2m_io_report(read_done).await?.into_inner();
    assert_eq!(result_read_done, Ack {});

    process.kill()?;
    Ok(())
}

#[tokio::test]
async fn integration_async_mp_1f_read2() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = Trace2eClient::connect("http://[::1]:8080").await?;
    let mut process = Command::new("tail").arg("-f").arg("/dev/null").spawn()?;
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    // CT declaration
    let file_creation = tonic::Request::new(LocalCt {
        process_id: process.id(),
        file_descriptor: 3,
        path: "bar.txt".to_string(),
    });
    let result_file_creation = client.p2m_local_enroll(file_creation).await?.into_inner();
    assert_eq!(result_file_creation, Ack {});

    // Read event
    let read_request = tonic::Request::new(IoInfo {
        process_id: process.id(),
        file_descriptor: 3,
        flow: Flow::Input.into(),
    });
    let result_read_request = client.p2m_io_request(read_request).await?;
    let grant_id = result_read_request.into_inner().id;

    // Read done
    let read_done = tonic::Request::new(IoResult {
        process_id: process.id(),
        file_descriptor: 3,
        grant_id,
        result: true,
    });
    let result_read_done = client.p2m_io_report(read_done).await?.into_inner();
    assert_eq!(result_read_done, Ack {});

    process.kill()?;
    Ok(())
}

#[tokio::test]
async fn integration_async_mp_1f_read3() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = Trace2eClient::connect("http://[::1]:8080").await?;
    let mut process = Command::new("tail").arg("-f").arg("/dev/null").spawn()?;
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    // CT declaration
    let file_creation = tonic::Request::new(LocalCt {
        process_id: process.id(),
        file_descriptor: 3,
        path: "bar.txt".to_string(),
    });
    let result_file_creation = client.p2m_local_enroll(file_creation).await?.into_inner();
    assert_eq!(result_file_creation, Ack {});

    // Read event
    let read_request = tonic::Request::new(IoInfo {
        process_id: process.id(),
        file_descriptor: 3,
        flow: Flow::Input.into(),
    });
    let result_read_request = client.p2m_io_request(read_request).await?;
    let grant_id = result_read_request.into_inner().id;

    // Read done
    let read_done = tonic::Request::new(IoResult {
        process_id: process.id(),
        file_descriptor: 3,
        grant_id,
        result: true,
    });
    let result_read_done = client.p2m_io_report(read_done).await?.into_inner();
    assert_eq!(result_read_done, Ack {});

    process.kill()?;
    Ok(())
}
