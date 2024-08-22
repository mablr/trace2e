use std::time::Duration;

use tokio::{
    sync::{mpsc, oneshot},
    time::timeout,
};

use crate::{
    containers::{containers_manager, ContainerAction, ContainerError},
    identifiers::Identifier,
};

use super::*;

#[test]
fn provenance_error_display_inconsistency() {
    let id = Identifier::File("/test/path/file1.txt".to_string());
    let error_container_not_registered =
        ProvenanceError::ContainerFailure(ContainerError::NotRegistered(id.clone()));
    let error_container_not_reserved =
        ProvenanceError::ContainerFailure(ContainerError::NotReserved(id.clone()));

    assert_eq!(
        format!("{}", error_container_not_registered),
        format!(
            "Provenance error: {}",
            ContainerError::NotRegistered(id.clone())
        )
    );

    assert_eq!(
        format!("{}", error_container_not_reserved),
        format!(
            "Provenance error: {}",
            ContainerError::NotReserved(id.clone())
        )
    );
}

#[tokio::test]
async fn provenance_layer_declare() -> Result<(), Box<dyn std::error::Error>> {
    let (sender, receiver) = mpsc::channel(32);
    tokio::spawn(containers_manager(receiver));
    let provenance = ProvenanceLayer::new(sender.clone());
    let source_identifier = Identifier::Process(1, 1);
    let destination_identifier = Identifier::File("/path/to/file1.txt".to_string());

    let (tx, rx) = oneshot::channel();
    sender
        .send(ContainerAction::Register(source_identifier.clone(), tx))
        .await?;
    rx.await?;
    let (tx, rx) = oneshot::channel();
    sender
        .send(ContainerAction::Register(
            destination_identifier.clone(),
            tx,
        ))
        .await?;
    rx.await?;

    provenance
        .declare_flow(source_identifier, destination_identifier)
        .await?;
    Ok(())
}

#[tokio::test]
async fn provenance_layer_record() -> Result<(), Box<dyn std::error::Error>> {
    let (sender, receiver) = mpsc::channel(32);
    tokio::spawn(containers_manager(receiver));
    let provenance = ProvenanceLayer::new(sender.clone());
    let source_identifier = Identifier::Process(1, 1);
    let destination_identifier = Identifier::File("/path/to/file1.txt".to_string());

    let (tx, rx) = oneshot::channel();
    sender
        .send(ContainerAction::Register(source_identifier.clone(), tx))
        .await?;
    rx.await?;
    let (tx, rx) = oneshot::channel();
    sender
        .send(ContainerAction::Register(
            destination_identifier.clone(),
            tx,
        ))
        .await?;
    rx.await?;

    let flow = provenance
        .declare_flow(source_identifier, destination_identifier)
        .await?;
    assert_eq!(provenance.record_flow(flow).await, Ok(()));
    Ok(())
}

#[tokio::test]
async fn provenance_layer_declare_errors() -> Result<(), Box<dyn std::error::Error>> {
    let (sender, receiver) = mpsc::channel(32);
    tokio::spawn(containers_manager(receiver));
    let provenance = ProvenanceLayer::new(sender.clone());
    let source_identifier = Identifier::Process(1, 1);
    let destination_identifier = Identifier::File("/path/to/file1.txt".to_string());

    assert_eq!(
        provenance
            .declare_flow(source_identifier.clone(), destination_identifier.clone())
            .await
            .unwrap_err(),
        ProvenanceError::ContainerFailure(ContainerError::NotRegistered(source_identifier.clone()))
    );

    let (tx, rx) = oneshot::channel();
    sender
        .send(ContainerAction::Register(source_identifier.clone(), tx))
        .await?;
    rx.await?;

    assert_eq!(
        provenance
            .declare_flow(source_identifier.clone(), destination_identifier.clone())
            .await
            .unwrap_err(),
        ProvenanceError::ContainerFailure(ContainerError::NotRegistered(
            destination_identifier.clone()
        ))
    );

    Ok(())
}

#[tokio::test]
async fn provenance_layer_record_errors() -> Result<(), Box<dyn std::error::Error>> {
    let (sender, receiver) = mpsc::channel(32);
    tokio::spawn(containers_manager(receiver));
    let provenance = ProvenanceLayer::new(sender.clone());
    let source_identifier = Identifier::Process(1, 1);
    let destination_identifier = Identifier::File("/path/to/file1.txt".to_string());
    let flow = Flow {
        id: 1,
        source: source_identifier.clone(),
        destination: destination_identifier.clone(),
    };

    assert_eq!(
        provenance.record_flow(flow.clone()).await.unwrap_err(),
        ProvenanceError::ContainerFailure(ContainerError::NotRegistered(source_identifier.clone()))
    );

    let (tx, rx) = oneshot::channel();
    sender
        .send(ContainerAction::Register(source_identifier.clone(), tx))
        .await?;
    rx.await?;

    assert_eq!(
        provenance.record_flow(flow.clone()).await.unwrap_err(),
        ProvenanceError::ContainerFailure(ContainerError::NotReserved(source_identifier.clone()))
    );

    let (tx, rx) = oneshot::channel();
    sender
        .send(ContainerAction::ReserveRead(source_identifier.clone(), tx))
        .await?;
    rx.await?;

    assert_eq!(
        provenance.record_flow(flow.clone()).await.unwrap_err(),
        ProvenanceError::ContainerFailure(ContainerError::NotRegistered(
            destination_identifier.clone()
        ))
    );

    let (tx, rx) = oneshot::channel();
    sender
        .send(ContainerAction::Register(
            destination_identifier.clone(),
            tx,
        ))
        .await?;
    rx.await?;
    let (tx, rx) = oneshot::channel();
    sender
        .send(ContainerAction::ReserveRead(source_identifier.clone(), tx))
        .await?;
    rx.await?;

    assert_eq!(
        provenance.record_flow(flow.clone()).await.unwrap_err(),
        ProvenanceError::ContainerFailure(ContainerError::NotReserved(
            destination_identifier.clone()
        ))
    );

    Ok(())
}

#[tokio::test]
async fn provenance_layer_declare_delayed() -> Result<(), Box<dyn std::error::Error>> {
    let (sender, receiver) = mpsc::channel(32);
    tokio::spawn(containers_manager(receiver));
    let provenance = ProvenanceLayer::new(sender.clone());
    let id1 = Identifier::Process(1, 1);
    let id2 = Identifier::File("/path/to/file1.txt".to_string());

    let (tx, rx) = oneshot::channel();
    sender
        .send(ContainerAction::Register(id1.clone(), tx))
        .await?;
    rx.await?;
    let (tx, rx) = oneshot::channel();
    sender
        .send(ContainerAction::Register(id2.clone(), tx))
        .await?;
    rx.await?;

    provenance.declare_flow(id1.clone(), id2.clone()).await?;

    // Flow declaration is delayed as the previous is no recorded so far
    assert!(timeout(
        Duration::from_millis(1),
        provenance.declare_flow(id1.clone(), id2.clone())
    )
    .await
    .is_err());

    Ok(())
}
