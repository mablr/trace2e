use std::{process::Command, time::Duration};

use procfs::process::Process;
use tokio::{
    sync::{mpsc, oneshot},
    time::timeout,
};

use crate::identifiers::Identifier;

use super::*;

#[tokio::test]
async fn unit_provenance_layer_declare_flow() -> Result<(), Box<dyn std::error::Error>> {
    let (sender, receiver) = mpsc::channel(32);

    tokio::spawn(provenance_layer(receiver));

    let id1 = Identifier::Process(1, 1);
    let id2 = Identifier::File("/path/to/file1.txt".to_string());

    let (tx, rx) = oneshot::channel();
    sender
        .send(ProvenanceAction::RegisterContainer(id1.clone(), tx))
        .await?;
    match rx.await.unwrap() {
        ProvenanceResult::Registered => (),
        _ => panic!("Container was expected to be registered successfully."),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(ProvenanceAction::RegisterContainer(id2.clone(), tx))
        .await?;
    match rx.await.unwrap() {
        ProvenanceResult::Registered => (),
        _ => panic!("Container was expected to be registered successfully."),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(ProvenanceAction::DeclareFlow(id1, id2, true, tx))
        .await?;
    match rx.await.unwrap() {
        ProvenanceResult::Declared(_) => (),
        _ => panic!("Flow was expected to be declared successfully."),
    };
    Ok(())
}

#[tokio::test]
async fn unit_provenance_layer_declare_missing_container() -> Result<(), Box<dyn std::error::Error>>
{
    let (sender, receiver) = mpsc::channel(32);

    tokio::spawn(provenance_layer(receiver));

    let id1 = Identifier::Process(1, 1);
    let id2 = Identifier::Process(2, 1);
    let id3 = Identifier::File("/path/to/file1.txt".to_string());

    let (tx, rx) = oneshot::channel();
    sender
        .send(ProvenanceAction::RegisterContainer(id1.clone(), tx))
        .await?;
    match rx.await.unwrap() {
        ProvenanceResult::Registered => (),
        _ => panic!("Container was expected to be registered successfully."),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(ProvenanceAction::DeclareFlow(
            id1.clone(),
            id3.clone(),
            false,
            tx,
        ))
        .await?;
    match rx.await.unwrap() {
        ProvenanceResult::Error(e) => {
            assert_eq!(
                format!("{}", e),
                format!(
                    "Provenance error: ({} || {}) are not registered.",
                    id1.clone(),
                    id3.clone()
                )
            );
        }
        _ => panic!("A Flow MissingRegistration was expected."),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(ProvenanceAction::DeclareFlow(
            id2.clone(),
            id3.clone(),
            false,
            tx,
        ))
        .await?;
    match rx.await.unwrap() {
        ProvenanceResult::Error(e) => {
            assert_eq!(
                format!("{}", e),
                format!(
                    "Provenance error: ({} || {}) are not registered.",
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
async fn unit_provenance_layer_declare_invalid_flow() -> Result<(), Box<dyn std::error::Error>> {
    let (sender, receiver) = mpsc::channel(32);

    tokio::spawn(provenance_layer(receiver));

    let id1 = Identifier::Process(1, 1);
    let id2 = Identifier::File("/path/to/file1.txt".to_string());

    let (tx, rx) = oneshot::channel();
    sender
        .send(ProvenanceAction::RegisterContainer(id1.clone(), tx))
        .await?;
    match rx.await.unwrap() {
        ProvenanceResult::Registered => (),
        _ => panic!("Container was expected to be registered successfully."),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(ProvenanceAction::RegisterContainer(id2.clone(), tx))
        .await?;
    match rx.await.unwrap() {
        ProvenanceResult::Registered => (),
        _ => panic!("Container was expected to be registered successfully."),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(ProvenanceAction::DeclareFlow(
            id1.clone(),
            id1.clone(),
            true,
            tx,
        ))
        .await?;
    match rx.await.unwrap() {
        ProvenanceResult::Error(e) => {
            assert_eq!(
                format!("{}", e),
                format!(
                    "Provenance error: {}<->{} Flow is invalid.",
                    id1.clone(),
                    id1.clone()
                )
            );
        }
        _ => panic!("Flow is supposed to be invalid"),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(ProvenanceAction::DeclareFlow(
            id2.clone(),
            id2.clone(),
            true,
            tx,
        ))
        .await?;
    match rx.await.unwrap() {
        ProvenanceResult::Error(e) => {
            assert_eq!(
                format!("{}", e),
                format!(
                    "Provenance error: {}<->{} Flow is invalid.",
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
async fn unit_provenance_layer_record() -> Result<(), Box<dyn std::error::Error>> {
    let (sender, receiver) = mpsc::channel(32);

    tokio::spawn(provenance_layer(receiver));

    let id1 = Identifier::Process(1, 1);
    let id2 = Identifier::File("/path/to/file1.txt".to_string());

    let (tx, rx) = oneshot::channel();
    sender
        .send(ProvenanceAction::RegisterContainer(id1.clone(), tx))
        .await?;
    match rx.await.unwrap() {
        ProvenanceResult::Registered => (),
        _ => panic!("Container was expected to be registered successfully."),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(ProvenanceAction::RegisterContainer(id2.clone(), tx))
        .await?;
    match rx.await.unwrap() {
        ProvenanceResult::Registered => (),
        _ => panic!("Container was expected to be registered successfully."),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(ProvenanceAction::DeclareFlow(id1, id2, true, tx))
        .await?;
    let grant_id = match rx.await.unwrap() {
        ProvenanceResult::Declared(grant_id) => grant_id,
        _ => panic!("Flow was expected to be declared successfully."),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(ProvenanceAction::RecordFlow(grant_id, tx))
        .await?;
    match rx.await.unwrap() {
        ProvenanceResult::Recorded => (),
        _ => panic!("Flow was expected to be recorded successfully."),
    };
    Ok(())
}

#[tokio::test]
async fn unit_provenance_layer_record_failure() -> Result<(), Box<dyn std::error::Error>> {
    let (sender, receiver) = mpsc::channel(32);

    tokio::spawn(provenance_layer(receiver));

    let (tx, rx) = oneshot::channel();
    sender.send(ProvenanceAction::RecordFlow(0, tx)).await?;
    match rx.await.unwrap() {
        ProvenanceResult::Error(e) => {
            assert_eq!(
                format!("{}", e),
                "Provenance error: unable to record Flow 0."
            );
        }
        _ => panic!("A Flow RecordingFailure was expected."),
    };
    Ok(())
}

#[tokio::test]
async fn unit_provenance_layer_declare_flow_delayed() -> Result<(), Box<dyn std::error::Error>> {
    let (sender, receiver) = mpsc::channel(32);

    tokio::spawn(provenance_layer(receiver));

    let id1 = Identifier::Process(1, 1);
    let id2 = Identifier::File("/path/to/file1.txt".to_string());

    let (tx, rx) = oneshot::channel();
    sender
        .send(ProvenanceAction::RegisterContainer(id1.clone(), tx))
        .await?;
    match rx.await.unwrap() {
        ProvenanceResult::Registered => (),
        _ => panic!("Container was expected to be registered successfully."),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(ProvenanceAction::RegisterContainer(id2.clone(), tx))
        .await?;
    match rx.await.unwrap() {
        ProvenanceResult::Registered => (),
        _ => panic!("Container was expected to be registered successfully."),
    };
    let (tx1, rx1) = oneshot::channel();
    sender
        .send(ProvenanceAction::DeclareFlow(
            id1.clone(),
            id2.clone(),
            false,
            tx1,
        ))
        .await?;
    let (tx2, rx2) = oneshot::channel();
    sender
        .send(ProvenanceAction::DeclareFlow(
            id1.clone(),
            id2.clone(),
            true,
            tx2,
        ))
        .await?;

    match rx1.await.unwrap() {
        ProvenanceResult::Declared(_) => (),
        _ => panic!("Flow was expected to be declared successfully."),
    }
    match timeout(Duration::from_millis(1), rx2).await {
        Err(_) => (), // Elapsed because flow declaration is supposed to be delayed
        _ => panic!("Flow was expected to be delayed."),
    }

    Ok(())
}

#[tokio::test]
async fn unit_provenance_layer_declare_flow_timeout_safe_release() -> Result<(), Box<dyn std::error::Error>> {
    let (sender, receiver) = mpsc::channel(32);
    tokio::spawn(provenance_layer(receiver));

    let mut process = Command::new("tail").arg("-f").arg("/dev/null").spawn()?;
    let process_starttime = match Process::new(process.id().try_into().unwrap())
        .and_then(|p| p.stat())
        .map(|stat| stat.starttime)
    {
        Ok(starttime) => starttime,
        Err(_) => panic!("Unable to get the test process starttime."),
    };

    let id1 = Identifier::Process(process.id(), process_starttime);
    let id2 = Identifier::File("/path/to/file1.txt".to_string());
    let id3 = Identifier::Process(1, 1);

    let (tx, rx) = oneshot::channel();
    sender
        .send(ProvenanceAction::RegisterContainer(id1.clone(), tx))
        .await?;
    match rx.await.unwrap() {
        ProvenanceResult::Registered => (),
        _ => panic!("Container was expected to be registered successfully."),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(ProvenanceAction::RegisterContainer(id2.clone(), tx))
        .await?;
    match rx.await.unwrap() {
        ProvenanceResult::Registered => (),
        _ => panic!("Container was expected to be registered successfully."),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(ProvenanceAction::RegisterContainer(id3.clone(), tx))
        .await?;
    match rx.await.unwrap() {
        ProvenanceResult::Registered => (),
        _ => panic!("Container was expected to be registered successfully."),
    };

    let (tx1, rx1) = oneshot::channel();
    sender
        .send(ProvenanceAction::DeclareFlow(
            id1.clone(),
            id2.clone(),
            false,
            tx1,
        ))
        .await?;
    let (tx2, rx2) = oneshot::channel();
    sender
        .send(ProvenanceAction::DeclareFlow(
            id3.clone(),
            id2.clone(),
            true,
            tx2,
        ))
        .await?;

    match rx1.await.unwrap() {
        ProvenanceResult::Declared(_) => (),
        _ => panic!("Flow was expected to be declared successfully."),
    }

    // This will make the previous reservation reach timeout.
    match rx2.await.unwrap() {
        ProvenanceResult::Declared(_) => (),
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
