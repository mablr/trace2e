use std::process::Command;

use trace2e_middleware::grpc_proto::{
    trace2e_client::Trace2eClient, Ack, Flow, IoInfo, IoResult, LocalCt, RemoteCt,
};

#[tokio::test]
async fn integration_sync_write() -> Result<(), Box<dyn std::error::Error>> {
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
        flow: Flow::Input.into(),
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
async fn integration_sync_read() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = Trace2eClient::connect("http://[::1]:8080").await?;
    let mut process = Command::new("tail").arg("-f").arg("/dev/null").spawn()?;
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    // CT declaration
    let file_creation = tonic::Request::new(LocalCt {
        process_id: process.id(),
        file_descriptor: 3,
        path: "foo.txt".to_string(),
    });
    let result_file_creation = client.p2m_local_enroll(file_creation).await?.into_inner();
    assert_eq!(result_file_creation, Ack {});

    // Read event
    let read_request = tonic::Request::new(IoInfo {
        process_id: process.id(),
        file_descriptor: 3,
        flow: Flow::Output.into(),
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
async fn integration_sync_complex() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = Trace2eClient::connect("http://[::1]:8080").await?;
    let mut process = Command::new("tail").arg("-f").arg("/dev/null").spawn()?;
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    // CT declaration
    let file_creation = tonic::Request::new(LocalCt {
        process_id: process.id(),
        file_descriptor: 3,
        path: "foobar.txt".to_string(),
    });
    let result_file_creation = client.p2m_local_enroll(file_creation).await?.into_inner();
    assert_eq!(result_file_creation, Ack {});

    // Write event
    let write_request1 = tonic::Request::new(IoInfo {
        process_id: process.id(),
        file_descriptor: 3,
        flow: Flow::Input.into(),
    });
    let result_write_request1 = client.p2m_io_request(write_request1).await?;
    let grant_id = result_write_request1.into_inner().id;

    // Write done
    let write_done1 = tonic::Request::new(IoResult {
        process_id: process.id(),
        file_descriptor: 3,
        grant_id,
        result: true,
    });
    let result_write_done1 = client.p2m_io_report(write_done1).await?.into_inner();
    assert_eq!(result_write_done1, Ack {});

    // Write event
    let write_request2 = tonic::Request::new(IoInfo {
        process_id: process.id(),
        file_descriptor: 3,
        flow: Flow::Input.into(),
    });
    let result_write_request2 = client.p2m_io_request(write_request2).await?;
    let grant_id = result_write_request2.into_inner().id;

    // Write done
    let write_done2 = tonic::Request::new(IoResult {
        process_id: process.id(),
        file_descriptor: 3,
        grant_id,
        result: true,
    });
    let result_write_done2 = client.p2m_io_report(write_done2).await?.into_inner();
    assert_eq!(result_write_done2, Ack {});

    // CT declaration
    let file_reopen = tonic::Request::new(LocalCt {
        process_id: process.id(),
        file_descriptor: 4,
        path: "foobar.txt".to_string(),
    });
    let result_file_reopen = client.p2m_local_enroll(file_reopen).await?.into_inner();
    assert_eq!(result_file_reopen, Ack {});

    // Seek event
    let seek_request = tonic::Request::new(IoInfo {
        process_id: process.id(),
        file_descriptor: 4,
        flow: Flow::None.into(),
    });
    let result_seek_request = client.p2m_io_request(seek_request).await.unwrap_err();
    assert_eq!(result_seek_request.message(), "Unsupported Flow type.");

    // seek done
    let seek_done = tonic::Request::new(IoResult {
        process_id: process.id(),
        file_descriptor: 4,
        grant_id: 0,
        result: true,
    });
    let result_seek_done = client.p2m_io_report(seek_done).await.unwrap_err();
    assert_eq!(
        result_seek_done.message(),
        "Traceability error: unable to record Flow 0."
    );

    // Read event
    let read_request = tonic::Request::new(IoInfo {
        process_id: process.id(),
        file_descriptor: 4,
        flow: Flow::Output.into(),
    });
    let result_read_request = client.p2m_io_request(read_request).await?;
    let grant_id = result_read_request.into_inner().id;

    // Read done
    let read_done = tonic::Request::new(IoResult {
        process_id: process.id(),
        file_descriptor: 4,
        grant_id,
        result: true,
    });
    let result_read_done = client.p2m_io_report(read_done).await?.into_inner();
    assert_eq!(result_read_done, Ack {});

    process.kill()?;
    Ok(())
}

#[tokio::test]
async fn integration_sync_stream() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = Trace2eClient::connect("http://[::1]:8080").await?;
    let mut process1 = Command::new("tail").arg("-f").arg("/dev/null").spawn()?;
    let mut process2 = Command::new("tail").arg("-f").arg("/dev/null").spawn()?;

    // CT declaration
    let local_file_creation = tonic::Request::new(LocalCt {
        process_id: process1.id(),
        file_descriptor: 3,
        path: "localfile.txt".to_string(),
    });
    let localside_stream_creation = tonic::Request::new(RemoteCt {
        process_id: process1.id(),
        file_descriptor: 4,
        local_socket: "127.0.0.1:8081".to_string(),
        peer_socket: "127.0.0.1:8082".to_string(),
    });
    let peer_file_creation = tonic::Request::new(LocalCt {
        process_id: process2.id(),
        file_descriptor: 5,
        path: "peerfile.txt".to_string(),
    });
    let peerside_stream_creation = tonic::Request::new(RemoteCt {
        process_id: process2.id(),
        file_descriptor: 6,
        local_socket: "127.0.0.1:8082".to_string(),
        peer_socket: "127.0.0.1:8081".to_string(),
    });
    assert_eq!(
        client.p2m_local_enroll(local_file_creation).await?.into_inner(),
        Ack {}
    );
    assert_eq!(
        client.p2m_local_enroll(peer_file_creation).await?.into_inner(),
        Ack {}
    );
    assert_eq!(
        client
            .p2m_remote_enroll(localside_stream_creation)
            .await?
            .into_inner(),
        Ack {}
    );
    assert_eq!(
        client
            .p2m_remote_enroll(peerside_stream_creation)
            .await?
            .into_inner(),
        Ack {}
    );

    // Process1 reads file
    let read_request1 = tonic::Request::new(IoInfo {
        process_id: process1.id(),
        file_descriptor: 3,
        flow: Flow::Input.into(),
    });
    let grant_id = client.p2m_io_request(read_request1).await?.into_inner().id;
    let read_done1 = tonic::Request::new(IoResult {
        process_id: process1.id(),
        file_descriptor: 3,
        grant_id,
        result: true,
    });
    assert_eq!(client.p2m_io_report(read_done1).await?.into_inner(), Ack {});

    // Process1 writes into stream
    let write_request1 = tonic::Request::new(IoInfo {
        process_id: process1.id(),
        file_descriptor: 4,
        flow: Flow::Output.into(),
    });
    let grant_id = client.p2m_io_request(write_request1).await?.into_inner().id;
    let write_done1 = tonic::Request::new(IoResult {
        process_id: process1.id(),
        file_descriptor: 4,
        grant_id,
        result: true,
    });
    assert_eq!(client.p2m_io_report(write_done1).await?.into_inner(), Ack {});

    // Process2 reads stream
    let read_request2 = tonic::Request::new(IoInfo {
        process_id: process2.id(),
        file_descriptor: 6,
        flow: Flow::Input.into(),
    });
    let grant_id = client.p2m_io_request(read_request2).await?.into_inner().id;
    let read_done2 = tonic::Request::new(IoResult {
        process_id: process2.id(),
        file_descriptor: 6,
        grant_id,
        result: true,
    });
    assert_eq!(client.p2m_io_report(read_done2).await?.into_inner(), Ack {});

    // Process2 writes into file
    let write_request2 = tonic::Request::new(IoInfo {
        process_id: process2.id(),
        file_descriptor: 5,
        flow: Flow::Output.into(),
    });
    let grant_id = client.p2m_io_request(write_request2).await?.into_inner().id;
    let write_done2 = tonic::Request::new(IoResult {
        process_id: process2.id(),
        file_descriptor: 5,
        grant_id,
        result: true,
    });
    assert_eq!(client.p2m_io_report(write_done2).await?.into_inner(), Ack {});

    process1.kill()?;
    process2.kill()?;
    Ok(())
}

#[tokio::test]
async fn integration_sync_stream_recycled_sockets() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = Trace2eClient::connect("http://[::1]:8080").await?;
    let mut process1 = Command::new("tail").arg("-f").arg("/dev/null").spawn()?;
    let mut process2 = Command::new("tail").arg("-f").arg("/dev/null").spawn()?;

    // CT declaration
    let local_file_creation = tonic::Request::new(LocalCt {
        process_id: process1.id(),
        file_descriptor: 3,
        path: "localfile.txt".to_string(),
    });
    let localside_stream_creation = tonic::Request::new(RemoteCt {
        process_id: process1.id(),
        file_descriptor: 4,
        local_socket: "127.0.0.1:8081".to_string(),
        peer_socket: "127.0.0.1:8082".to_string(),
    });
    let peer_file_creation = tonic::Request::new(LocalCt {
        process_id: process2.id(),
        file_descriptor: 5,
        path: "peerfile.txt".to_string(),
    });
    let peerside_stream_creation = tonic::Request::new(RemoteCt {
        process_id: process2.id(),
        file_descriptor: 6,
        local_socket: "127.0.0.1:8082".to_string(),
        peer_socket: "127.0.0.1:8081".to_string(),
    });
    assert_eq!(
        client.p2m_local_enroll(local_file_creation).await?.into_inner(),
        Ack {}
    );
    assert_eq!(
        client.p2m_local_enroll(peer_file_creation).await?.into_inner(),
        Ack {}
    );
    assert_eq!(
        client
            .p2m_remote_enroll(localside_stream_creation)
            .await?
            .into_inner(),
        Ack {}
    );
    assert_eq!(
        client
            .p2m_remote_enroll(peerside_stream_creation)
            .await?
            .into_inner(),
        Ack {}
    );

    // Process1 reads file
    let read_request1 = tonic::Request::new(IoInfo {
        process_id: process1.id(),
        file_descriptor: 3,
        flow: Flow::Input.into(),
    });
    let grant_id = client.p2m_io_request(read_request1).await?.into_inner().id;
    let read_done1 = tonic::Request::new(IoResult {
        process_id: process1.id(),
        file_descriptor: 3,
        grant_id,
        result: true,
    });
    assert_eq!(client.p2m_io_report(read_done1).await?.into_inner(), Ack {});

    // Process1 writes into stream
    let write_request1 = tonic::Request::new(IoInfo {
        process_id: process1.id(),
        file_descriptor: 4,
        flow: Flow::Output.into(),
    });
    let grant_id = client.p2m_io_request(write_request1).await?.into_inner().id;
    let write_done1 = tonic::Request::new(IoResult {
        process_id: process1.id(),
        file_descriptor: 4,
        grant_id,
        result: true,
    });
    assert_eq!(client.p2m_io_report(write_done1).await?.into_inner(), Ack {});

    // Process2 reads stream
    let read_request2 = tonic::Request::new(IoInfo {
        process_id: process2.id(),
        file_descriptor: 6,
        flow: Flow::Input.into(),
    });
    let grant_id = client.p2m_io_request(read_request2).await?.into_inner().id;
    let read_done2 = tonic::Request::new(IoResult {
        process_id: process2.id(),
        file_descriptor: 6,
        grant_id,
        result: true,
    });
    assert_eq!(client.p2m_io_report(read_done2).await?.into_inner(), Ack {});

    // Process2 writes into file
    let write_request2 = tonic::Request::new(IoInfo {
        process_id: process2.id(),
        file_descriptor: 5,
        flow: Flow::Output.into(),
    });
    let grant_id = client.p2m_io_request(write_request2).await?.into_inner().id;
    let write_done2 = tonic::Request::new(IoResult {
        process_id: process2.id(),
        file_descriptor: 5,
        grant_id,
        result: true,
    });
    assert_eq!(client.p2m_io_report(write_done2).await?.into_inner(), Ack {});

    process1.kill()?;
    process2.kill()?;

    ////////////////////////////////////////////////////////////////////////////
    // Recycling sockets with new processes
    ////////////////////////////////////////////////////////////////////////////

    let mut process1 = Command::new("tail").arg("-f").arg("/dev/null").spawn()?;
    let mut process2 = Command::new("tail").arg("-f").arg("/dev/null").spawn()?;

    // CT declaration
    let local_file_creation = tonic::Request::new(LocalCt {
        process_id: process1.id(),
        file_descriptor: 3,
        path: "localfile.txt".to_string(),
    });
    let localside_stream_creation = tonic::Request::new(RemoteCt {
        process_id: process1.id(),
        file_descriptor: 4,
        local_socket: "127.0.0.1:8081".to_string(),
        peer_socket: "127.0.0.1:8082".to_string(),
    });
    let peer_file_creation = tonic::Request::new(LocalCt {
        process_id: process2.id(),
        file_descriptor: 5,
        path: "peerfile.txt".to_string(),
    });
    let peerside_stream_creation = tonic::Request::new(RemoteCt {
        process_id: process2.id(),
        file_descriptor: 6,
        local_socket: "127.0.0.1:8082".to_string(),
        peer_socket: "127.0.0.1:8081".to_string(),
    });
    assert_eq!(
        client.p2m_local_enroll(local_file_creation).await?.into_inner(),
        Ack {}
    );
    assert_eq!(
        client.p2m_local_enroll(peer_file_creation).await?.into_inner(),
        Ack {}
    );
    assert_eq!(
        client
            .p2m_remote_enroll(localside_stream_creation)
            .await?
            .into_inner(),
        Ack {}
    );
    assert_eq!(
        client
            .p2m_remote_enroll(peerside_stream_creation)
            .await?
            .into_inner(),
        Ack {}
    );

    // Nothing is written to the stream by new Process1

    // New Process2 reads stream: empty buffer expected, no provenance info must be forwarded
    let read_request2 = tonic::Request::new(IoInfo {
        process_id: process2.id(),
        file_descriptor: 6,
        flow: Flow::Input.into(),
    });
    let grant_id = client.p2m_io_request(read_request2).await?.into_inner().id;
    let read_done2 = tonic::Request::new(IoResult {
        process_id: process2.id(),
        file_descriptor: 6,
        grant_id,
        result: true,
    });
    assert_eq!(client.p2m_io_report(read_done2).await?.into_inner(), Ack {});

    process1.kill()?;
    process2.kill()?;

    Ok(())
}
