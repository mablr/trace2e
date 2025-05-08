use std::{process::Command, time::Duration};
use procfs::process::Process;
use tokio::{sync::oneshot, time::timeout};

use crate::identifier::Identifier;

use super::*;

#[tokio::test]
async fn unit_traceability_server_declare_flow() -> Result<(), Box<dyn std::error::Error>> {
    let sender = spawn_traceability_server();

    let id1 = Identifier::new_process(1, 1, String::new());
    let id2 = Identifier::new_file("/path/to/file1.txt".to_string());

    let (tx, rx) = oneshot::channel();
    sender
        .send(TraceabilityRequest::RegisterContainer(id1.clone(), tx))
        .await?;
    match rx.await.unwrap() {
        TraceabilityResponse::Registered(id) => assert_eq!(id, id1),
        _ => panic!("Container was expected to be registered successfully."),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(TraceabilityRequest::RegisterContainer(id2.clone(), tx))
        .await?;
    match rx.await.unwrap() {
        TraceabilityResponse::Registered(id) => assert_eq!(id, id2),
        _ => panic!("Container was expected to be registered successfully."),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(TraceabilityRequest::DeclareFlow(id1, id2, true, tx))
        .await?;
    match rx.await.unwrap() {
        TraceabilityResponse::Declared(_) => (),
        _ => panic!("Flow was expected to be declared successfully."),
    };
    Ok(())
}

#[tokio::test]
async fn unit_traceability_server_declare_missing_container(
) -> Result<(), Box<dyn std::error::Error>> {
    let sender = spawn_traceability_server();

    let id1 = Identifier::new_process(1, 1, String::new());
    let id2 = Identifier::new_process(2, 1, String::new());
    let id3 = Identifier::new_file("/path/to/file1.txt".to_string());

    let (tx, rx) = oneshot::channel();
    sender
        .send(TraceabilityRequest::RegisterContainer(id1.clone(), tx))
        .await?;
    match rx.await.unwrap() {
        TraceabilityResponse::Registered(id) => assert_eq!(id, id1),
        _ => panic!("Container was expected to be registered successfully."),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(TraceabilityRequest::DeclareFlow(
            id1.clone(),
            id3.clone(),
            false,
            tx,
        ))
        .await?;
    match rx.await.unwrap() {
        TraceabilityResponse::Error(e) => {
            assert_eq!(
                format!("{}", e),
                format!("Traceability error: {:?} is not registered.", id3.clone())
            );
        }
        _ => panic!("A Flow MissingRegistration was expected."),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(TraceabilityRequest::DeclareFlow(
            id2.clone(),
            id3.clone(),
            false,
            tx,
        ))
        .await?;
    match rx.await.unwrap() {
        TraceabilityResponse::Error(e) => {
            assert_eq!(
                format!("{}", e),
                format!(
                    "Traceability error: ({:?} || {:?}) are not registered.",
                    id2.clone(),
                    id3.clone()
                )
            );
        }
        _ => panic!("A Flow MissingRegistration was expected."),
    };

    Ok(())
}

#[tokio::test]
async fn unit_traceability_server_declare_invalid_flow() -> Result<(), Box<dyn std::error::Error>> {
    let sender = spawn_traceability_server();

    let id1 = Identifier::new_process(1, 1, String::new());
    let id2 = Identifier::new_file("/path/to/file1.txt".to_string());

    let (tx, rx) = oneshot::channel();
    sender
        .send(TraceabilityRequest::RegisterContainer(id1.clone(), tx))
        .await?;
    match rx.await.unwrap() {
        TraceabilityResponse::Registered(id) => assert_eq!(id, id1),
        _ => panic!("Container was expected to be registered successfully."),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(TraceabilityRequest::RegisterContainer(id2.clone(), tx))
        .await?;
    match rx.await.unwrap() {
        TraceabilityResponse::Registered(id) => assert_eq!(id, id2),
        _ => panic!("Container was expected to be registered successfully."),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(TraceabilityRequest::DeclareFlow(
            id1.clone(),
            id1.clone(),
            true,
            tx,
        ))
        .await?;
    match rx.await.unwrap() {
        TraceabilityResponse::Error(e) => {
            assert_eq!(
                format!("{}", e),
                format!(
                    "Traceability error: {:?}<->{:?} Flow is invalid.",
                    id1.clone(),
                    id1.clone()
                )
            );
        }
        _ => panic!("Flow is supposed to be invalid"),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(TraceabilityRequest::DeclareFlow(
            id2.clone(),
            id2.clone(),
            true,
            tx,
        ))
        .await?;
    match rx.await.unwrap() {
        TraceabilityResponse::Error(e) => {
            assert_eq!(
                format!("{}", e),
                format!(
                    "Traceability error: {:?}<->{:?} Flow is invalid.",
                    id2.clone(),
                    id2.clone()
                )
            );
        }
        _ => panic!("Flow is supposed to be invalid"),
    };

    Ok(())
}

#[tokio::test]
async fn unit_traceability_server_record() -> Result<(), Box<dyn std::error::Error>> {
    let sender = spawn_traceability_server();

    let id1 = Identifier::new_process(1, 1, String::new());
    let id2 = Identifier::new_file("/path/to/file1.txt".to_string());

    let (tx, rx) = oneshot::channel();
    sender
        .send(TraceabilityRequest::RegisterContainer(id1.clone(), tx))
        .await?;
    match rx.await.unwrap() {
        TraceabilityResponse::Registered(id) => assert_eq!(id, id1),
        _ => panic!("Container was expected to be registered successfully."),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(TraceabilityRequest::RegisterContainer(id2.clone(), tx))
        .await?;
    match rx.await.unwrap() {
        TraceabilityResponse::Registered(id) => assert_eq!(id, id2),
        _ => panic!("Container was expected to be registered successfully."),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(TraceabilityRequest::DeclareFlow(id1, id2, true, tx))
        .await?;
    let grant_id = match rx.await.unwrap() {
        TraceabilityResponse::Declared(grant_id) => grant_id,
        _ => panic!("Flow was expected to be declared successfully."),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(TraceabilityRequest::RecordFlow(grant_id, tx))
        .await?;
    match rx.await.unwrap() {
        TraceabilityResponse::Recorded => (),
        _ => panic!("Flow was expected to be recorded successfully."),
    };
    Ok(())
}

#[tokio::test]
async fn unit_traceability_server_record_failure() -> Result<(), Box<dyn std::error::Error>> {
    let sender = spawn_traceability_server();

    let (tx, rx) = oneshot::channel();
    sender.send(TraceabilityRequest::RecordFlow(0, tx)).await?;
    match rx.await.unwrap() {
        TraceabilityResponse::Error(e) => {
            assert_eq!(
                format!("{}", e),
                "Traceability error: unable to record Flow 0."
            );
        }
        _ => panic!("A Flow RecordingFailure was expected."),
    };
    Ok(())
}

#[tokio::test]
async fn unit_traceability_server_declare_flow_delayed() -> Result<(), Box<dyn std::error::Error>> {
    let sender = spawn_traceability_server();

    let id1 = Identifier::new_process(1, 1, String::new());
    let id2 = Identifier::new_file("/path/to/file1.txt".to_string());

    let (tx, rx) = oneshot::channel();
    sender
        .send(TraceabilityRequest::RegisterContainer(id1.clone(), tx))
        .await?;
    match rx.await.unwrap() {
        TraceabilityResponse::Registered(id) => assert_eq!(id, id1),
        _ => panic!("Container was expected to be registered successfully."),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(TraceabilityRequest::RegisterContainer(id2.clone(), tx))
        .await?;
    match rx.await.unwrap() {
        TraceabilityResponse::Registered(id) => assert_eq!(id, id2),
        _ => panic!("Container was expected to be registered successfully."),
    };
    let (tx1, rx1) = oneshot::channel();
    sender
        .send(TraceabilityRequest::DeclareFlow(
            id1.clone(),
            id2.clone(),
            false,
            tx1,
        ))
        .await?;
    let (tx2, rx2) = oneshot::channel();
    sender
        .send(TraceabilityRequest::DeclareFlow(
            id1.clone(),
            id2.clone(),
            true,
            tx2,
        ))
        .await?;

    match rx1.await.unwrap() {
        TraceabilityResponse::Declared(_) => (),
        _ => panic!("Flow was expected to be declared successfully."),
    }
    match timeout(Duration::from_millis(1), rx2).await {
        Err(_) => (), // Elapsed because flow declaration is supposed to be delayed
        _ => panic!("Flow was expected to be delayed."),
    }

    Ok(())
}

#[tokio::test]
async fn unit_traceability_server_forced_release() -> Result<(), Box<dyn std::error::Error>> {
    let sender = spawn_traceability_server();

    let mut process = Command::new("tail").arg("-f").arg("/dev/null").spawn()?;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let process_starttime = match Process::new(process.id().try_into().unwrap())
        .and_then(|p| p.stat())
        .map(|stat| stat.starttime)
    {
        Ok(starttime) => starttime,
        Err(_) => panic!("Unable to get the test process starttime."),
    };
    let process_exe_path =
        match Process::new(process.id().try_into().unwrap()).and_then(|p| p.exe()) {
            Ok(path_buf) => path_buf.to_str().unwrap_or("").to_string(),
            Err(_) => panic!("Unable to get the test process exe path."),
        };

    let id1 = Identifier::new_process(process.id(), process_starttime, process_exe_path);
    let id2 = Identifier::new_file("/path/to/file1.txt".to_string());
    let id3 = Identifier::new_process(1, 1, String::new());

    let (tx, rx) = oneshot::channel();
    sender
        .send(TraceabilityRequest::RegisterContainer(id1.clone(), tx))
        .await?;
    match rx.await.unwrap() {
        TraceabilityResponse::Registered(id) => assert_eq!(id, id1),
        _ => panic!("Container was expected to be registered successfully."),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(TraceabilityRequest::RegisterContainer(id2.clone(), tx))
        .await?;
    match rx.await.unwrap() {
        TraceabilityResponse::Registered(id) => assert_eq!(id, id2),
        _ => panic!("Container was expected to be registered successfully."),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(TraceabilityRequest::RegisterContainer(id3.clone(), tx))
        .await?;
    match rx.await.unwrap() {
        TraceabilityResponse::Registered(id) => assert_eq!(id, id3),
        _ => panic!("Container was expected to be registered successfully."),
    };

    let (tx1, rx1) = oneshot::channel();
    sender
        .send(TraceabilityRequest::DeclareFlow(
            id1.clone(),
            id2.clone(),
            false,
            tx1,
        ))
        .await?;
    let (tx2, rx2) = oneshot::channel();
    sender
        .send(TraceabilityRequest::DeclareFlow(
            id3.clone(),
            id2.clone(),
            true,
            tx2,
        ))
        .await?;

    match rx1.await.unwrap() {
        TraceabilityResponse::Declared(_) => (),
        _ => panic!("Flow was expected to be declared successfully."),
    }

    // This will make the previous reservation reach timeout.
    match rx2.await.unwrap() {
        TraceabilityResponse::Declared(_) => (),
        _ => panic!("Flow was expected to be declared successfully."),
    }

    match process.try_wait() {
        Ok(Some(result)) => assert_eq!(format!("{:?}", result), "ExitStatus(unix_wait_status(9))"),
        Ok(None) => {
            process.kill().unwrap();
            panic!("Process was supposed to be killed by the provenance layer after timeout");
        }
        Err(_) => panic!("Unable to get ExitStatus of process."),
    }
    Ok(())
}
