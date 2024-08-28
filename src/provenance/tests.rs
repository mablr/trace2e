use std::time::Duration;

use tokio::{sync::{mpsc, oneshot}, time::timeout};

use crate::identifiers::Identifier;

use super::*;

#[tokio::test]
async fn provenance_layer_declare() -> Result<(), Box<dyn std::error::Error>> {
    let (sender, receiver) = mpsc::channel(32);

    tokio::spawn(provenance_layer(receiver));

    let source_identifier = Identifier::Process(1, 1);
    let destination_identifier = Identifier::File("/path/to/file1.txt".to_string());

    let (tx, rx) = oneshot::channel();
    sender
        .send(ProvenanceAction::RegisterContainer(
            source_identifier.clone(),
            tx,
        ))
        .await?;
    match rx.await.unwrap() {
        ProvenanceResult::Registered => (),
        _ => panic!(),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(ProvenanceAction::RegisterContainer(
            destination_identifier.clone(),
            tx,
        ))
        .await?;
    match rx.await.unwrap() {
        ProvenanceResult::Registered => (),
        _ => panic!(),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(ProvenanceAction::DeclareFlow(
            source_identifier,
            destination_identifier,
            true,
            tx,
        ))
        .await?;
    match rx.await.unwrap() {
        ProvenanceResult::Declared(_) => (),
        _ => panic!(),
    };
    Ok(())
}

#[tokio::test]
async fn provenance_layer_record() -> Result<(), Box<dyn std::error::Error>> {
    let (sender, receiver) = mpsc::channel(32);

    tokio::spawn(provenance_layer(receiver));

    let source_identifier = Identifier::Process(1, 1);
    let destination_identifier = Identifier::File("/path/to/file1.txt".to_string());

    let (tx, rx) = oneshot::channel();
    sender
        .send(ProvenanceAction::RegisterContainer(
            source_identifier.clone(),
            tx,
        ))
        .await?;
    match rx.await.unwrap() {
        ProvenanceResult::Registered => (),
        _ => panic!(),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(ProvenanceAction::RegisterContainer(
            destination_identifier.clone(),
            tx,
        ))
        .await?;
    match rx.await.unwrap() {
        ProvenanceResult::Registered => (),
        _ => panic!(),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(ProvenanceAction::DeclareFlow(
            source_identifier,
            destination_identifier,
            true,
            tx,
        ))
        .await?;
    let grant_id = match rx.await.unwrap() {
        ProvenanceResult::Declared(grant_id) => grant_id,
        _ => panic!(),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(ProvenanceAction::RecordFlow(grant_id, tx))
        .await?;
    match rx.await.unwrap() {
        ProvenanceResult::Recorded => (),
        _ => panic!(),
    };
    Ok(())
}

#[tokio::test]
async fn provenance_layer_declare_delayed() -> Result<(), Box<dyn std::error::Error>> {
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
        _ => panic!(),
    };

    let (tx, rx) = oneshot::channel();
    sender
        .send(ProvenanceAction::RegisterContainer(id2.clone(), tx))
        .await?;
    match rx.await.unwrap() {
        ProvenanceResult::Registered => (),
        _ => panic!(),
    };
    let (tx, rx) = oneshot::channel();
    sender
        .send(ProvenanceAction::DeclareFlow(
            id1.clone(),
            id2.clone(),
            true,
            tx,
        ))
        .await?;
    match rx.await.unwrap() {
        ProvenanceResult::Declared(_) => (),
        _ => panic!(),
    }

    let (tx, rx) = oneshot::channel();
    sender
        .send(ProvenanceAction::DeclareFlow(
            id1.clone(),
            id2.clone(),
            true,
            tx,
        ))
        .await?;
    
    match timeout(Duration::from_millis(1), rx).await {
        Err(_) => (), // Elapsed because flow declaration is supposed to be delayed
        _ => panic!()
    }

    Ok(())
}
