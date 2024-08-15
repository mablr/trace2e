use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use tokio::sync::{
    mpsc,
    oneshot::{self, error::RecvError},
};

use crate::identifiers::Identifier;

use super::*;

#[test]
fn containers_manager_register() {
    let mut containers_manager = ContainersManager::default();
    let id1 = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::Stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    let id3 = Identifier::Process(1, 1);

    assert_eq!(containers_manager.register(id1.clone()), true);
    assert_eq!(containers_manager.register(id2.clone()), true);
    assert_eq!(containers_manager.register(id1.clone()), false);
    assert_eq!(containers_manager.register(id3.clone()), true);
    assert_eq!(containers_manager.register(id3.clone()), false);
    assert_eq!(containers_manager.register(id2.clone()), false);
    assert_eq!(containers_manager.register(id1.clone()), false);
}

#[test]
fn containers_manager_register_recycled_pid() {
    let mut containers_manager = ContainersManager::default();

    let id1 = Identifier::Process(1, 1);
    let id2 = Identifier::Process(1, 2);

    assert_eq!(containers_manager.register(id1.clone()), true);
    assert_eq!(containers_manager.register(id1.clone()), false);
    assert_eq!(containers_manager.register(id2.clone()), true);
    assert_eq!(containers_manager.register(id2.clone()), false);
}

#[test]
fn containers_manager_try_read() {
    let mut containers_manager = ContainersManager::default();
    let id1 = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::Stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    let id3 = Identifier::Process(1, 1);

    assert_eq!(containers_manager.register(id1.clone()), true);
    assert_eq!(containers_manager.register(id2.clone()), true);
    assert_eq!(containers_manager.register(id3.clone()), true);

    assert_eq!(containers_manager.try_read(id1.clone()), Ok(true));
    assert_eq!(containers_manager.try_read(id2.clone()), Ok(true));
    assert_eq!(containers_manager.try_read(id3.clone()), Ok(true));
}

#[test]
fn containers_manager_try_read_unregistered() {
    let mut containers_manager = ContainersManager::default();
    let id1 = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::Stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    let id3 = Identifier::Process(1, 1);

    assert_eq!(
        containers_manager.try_read(id1.clone()),
        Err(ContainerError::NotRegistered(id1.clone()))
    );
    assert_eq!(
        containers_manager.try_read(id2.clone()),
        Err(ContainerError::NotRegistered(id2.clone()))
    );
    assert_eq!(
        containers_manager.try_read(id3.clone()),
        Err(ContainerError::NotRegistered(id3.clone()))
    );
}

#[test]
fn containers_manager_try_read_already_read() {
    let mut containers_manager = ContainersManager::default();
    let id1 = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::Stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    let id3 = Identifier::Process(1, 1);

    assert_eq!(containers_manager.register(id1.clone()), true);
    assert_eq!(containers_manager.register(id2.clone()), true);
    assert_eq!(containers_manager.register(id3.clone()), true);

    assert_eq!(containers_manager.try_read(id1.clone()), Ok(true));
    assert_eq!(containers_manager.try_read(id2.clone()), Ok(true));
    assert_eq!(containers_manager.try_read(id3.clone()), Ok(true));

    assert_eq!(containers_manager.try_read(id1.clone()), Ok(true));
    assert_eq!(containers_manager.try_read(id2.clone()), Ok(true));
    assert_eq!(containers_manager.try_read(id3.clone()), Ok(true));
}

#[test]
fn containers_manager_try_read_already_write() {
    let mut containers_manager = ContainersManager::default();
    let id1 = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::Stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    let id3 = Identifier::Process(1, 1);

    assert_eq!(containers_manager.register(id1.clone()), true);
    assert_eq!(containers_manager.register(id2.clone()), true);
    assert_eq!(containers_manager.register(id3.clone()), true);

    assert_eq!(containers_manager.try_write(id1.clone()), Ok(true));
    assert_eq!(containers_manager.try_write(id2.clone()), Ok(true));
    assert_eq!(containers_manager.try_write(id3.clone()), Ok(true));

    assert_eq!(containers_manager.try_read(id1.clone()), Ok(false));
    assert_eq!(containers_manager.try_read(id2.clone()), Ok(false));
    assert_eq!(containers_manager.try_read(id3.clone()), Ok(false));
}

#[test]
fn containers_manager_try_write() {
    let mut containers_manager = ContainersManager::default();
    let id1 = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::Stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    let id3 = Identifier::Process(1, 1);

    assert_eq!(containers_manager.register(id1.clone()), true);
    assert_eq!(containers_manager.register(id2.clone()), true);
    assert_eq!(containers_manager.register(id3.clone()), true);

    assert_eq!(containers_manager.try_write(id1.clone()), Ok(true));
    assert_eq!(containers_manager.try_write(id2.clone()), Ok(true));
    assert_eq!(containers_manager.try_write(id3.clone()), Ok(true));
}

#[test]
fn containers_manager_try_write_unregistered() {
    let mut containers_manager = ContainersManager::default();
    let id1 = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::Stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    let id3 = Identifier::Process(1, 1);

    assert_eq!(
        containers_manager.try_write(id1.clone()),
        Err(ContainerError::NotRegistered(id1.clone()))
    );
    assert_eq!(
        containers_manager.try_write(id2.clone()),
        Err(ContainerError::NotRegistered(id2.clone()))
    );
    assert_eq!(
        containers_manager.try_write(id3.clone()),
        Err(ContainerError::NotRegistered(id3.clone()))
    );
}

#[test]
fn containers_manager_try_write_already_read() {
    let mut containers_manager = ContainersManager::default();
    let id1 = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::Stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    let id3 = Identifier::Process(1, 1);

    assert_eq!(containers_manager.register(id1.clone()), true);
    assert_eq!(containers_manager.register(id2.clone()), true);
    assert_eq!(containers_manager.register(id3.clone()), true);

    assert_eq!(containers_manager.try_read(id1.clone()), Ok(true));
    assert_eq!(containers_manager.try_read(id2.clone()), Ok(true));
    assert_eq!(containers_manager.try_read(id3.clone()), Ok(true));

    assert_eq!(containers_manager.try_write(id1.clone()), Ok(false));
    assert_eq!(containers_manager.try_write(id2.clone()), Ok(false));
    assert_eq!(containers_manager.try_write(id3.clone()), Ok(false));
}

#[test]
fn containers_manager_try_write_already_write() {
    let mut containers_manager = ContainersManager::default();
    let id1 = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::Stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    let id3 = Identifier::Process(1, 1);

    assert_eq!(containers_manager.register(id1.clone()), true);
    assert_eq!(containers_manager.register(id2.clone()), true);
    assert_eq!(containers_manager.register(id3.clone()), true);

    assert_eq!(containers_manager.try_write(id1.clone()), Ok(true));
    assert_eq!(containers_manager.try_write(id2.clone()), Ok(true));
    assert_eq!(containers_manager.try_write(id3.clone()), Ok(true));

    assert_eq!(containers_manager.try_write(id1.clone()), Ok(false));
    assert_eq!(containers_manager.try_write(id2.clone()), Ok(false));
    assert_eq!(containers_manager.try_write(id3.clone()), Ok(false));
}

#[test]
fn containers_manager_try_release_write() {
    let mut containers_manager = ContainersManager::default();
    let id1 = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::Stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    let id3 = Identifier::Process(1, 1);

    assert_eq!(containers_manager.register(id1.clone()), true);
    assert_eq!(containers_manager.register(id2.clone()), true);
    assert_eq!(containers_manager.register(id3.clone()), true);

    assert_eq!(containers_manager.try_write(id1.clone()), Ok(true));
    assert_eq!(containers_manager.try_write(id2.clone()), Ok(true));
    assert_eq!(containers_manager.try_write(id3.clone()), Ok(true));

    assert_eq!(containers_manager.try_release(id1.clone()), Ok(()));
    assert_eq!(containers_manager.try_release(id2.clone()), Ok(()));
    assert_eq!(containers_manager.try_release(id3.clone()), Ok(()));

    assert_eq!(containers_manager.try_write(id1.clone()), Ok(true));
    assert_eq!(containers_manager.try_write(id2.clone()), Ok(true));
    assert_eq!(containers_manager.try_write(id3.clone()), Ok(true));
}

#[test]
fn containers_manager_try_release_read() {
    let mut containers_manager = ContainersManager::default();
    let id1 = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::Stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    let id3 = Identifier::Process(1, 1);

    assert_eq!(containers_manager.register(id1.clone()), true);
    assert_eq!(containers_manager.register(id2.clone()), true);
    assert_eq!(containers_manager.register(id3.clone()), true);

    assert_eq!(containers_manager.try_read(id1.clone()), Ok(true));
    assert_eq!(containers_manager.try_read(id2.clone()), Ok(true));
    assert_eq!(containers_manager.try_read(id3.clone()), Ok(true));

    assert_eq!(containers_manager.try_release(id1.clone()), Ok(()));
    assert_eq!(containers_manager.try_release(id2.clone()), Ok(()));
    assert_eq!(containers_manager.try_release(id3.clone()), Ok(()));

    assert_eq!(containers_manager.try_write(id1.clone()), Ok(true));
    assert_eq!(containers_manager.try_write(id2.clone()), Ok(true));
    assert_eq!(containers_manager.try_write(id3.clone()), Ok(true));
}

#[test]
fn containers_manager_try_release_available() {
    let mut containers_manager = ContainersManager::default();
    let id1 = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::Stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    let id3 = Identifier::Process(1, 1);

    assert_eq!(containers_manager.register(id1.clone()), true);
    assert_eq!(containers_manager.register(id2.clone()), true);
    assert_eq!(containers_manager.register(id3.clone()), true);

    assert_eq!(
        containers_manager.try_release(id1.clone()),
        Err(ContainerError::NotReserved(id1.clone()))
    );
    assert_eq!(
        containers_manager.try_release(id2.clone()),
        Err(ContainerError::NotReserved(id2.clone()))
    );
    assert_eq!(
        containers_manager.try_release(id3.clone()),
        Err(ContainerError::NotReserved(id3.clone()))
    );
}

#[test]
fn containers_manager_try_release_not_registered() {
    let mut containers_manager = ContainersManager::default();
    let id1 = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::Stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    let id3 = Identifier::Process(1, 1);

    assert_eq!(
        containers_manager.try_read(id1.clone()),
        Err(ContainerError::NotRegistered(id1.clone()))
    );
    assert_eq!(
        containers_manager.try_write(id1.clone()),
        Err(ContainerError::NotRegistered(id1.clone()))
    );
    assert_eq!(
        containers_manager.try_release(id1.clone()),
        Err(ContainerError::NotRegistered(id1.clone()))
    );
    assert_eq!(
        containers_manager.try_read(id2.clone()),
        Err(ContainerError::NotRegistered(id2.clone()))
    );
    assert_eq!(
        containers_manager.try_write(id2.clone()),
        Err(ContainerError::NotRegistered(id2.clone()))
    );
    assert_eq!(
        containers_manager.try_release(id2.clone()),
        Err(ContainerError::NotRegistered(id2.clone()))
    );
    assert_eq!(
        containers_manager.try_read(id3.clone()),
        Err(ContainerError::NotRegistered(id3.clone()))
    );
    assert_eq!(
        containers_manager.try_write(id3.clone()),
        Err(ContainerError::NotRegistered(id3.clone()))
    );
    assert_eq!(
        containers_manager.try_release(id3.clone()),
        Err(ContainerError::NotRegistered(id3.clone()))
    );
}

#[test]
fn container_error_display_not_registered() {
    let mut containers_manager = ContainersManager::default();
    let id1 = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::Stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    let id3 = Identifier::Process(1, 1);

    assert_eq!(
        format!("{}", containers_manager.try_read(id1.clone()).unwrap_err()),
        format!("{}", ContainerError::NotRegistered(id1.clone()))
    );
    assert_eq!(
        format!("{}", containers_manager.try_write(id1.clone()).unwrap_err()),
        format!("{}", ContainerError::NotRegistered(id1.clone()))
    );
    assert_eq!(
        format!(
            "{}",
            containers_manager.try_release(id1.clone()).unwrap_err()
        ),
        format!("{}", ContainerError::NotRegistered(id1.clone()))
    );
    assert_eq!(
        format!("{}", containers_manager.try_read(id2.clone()).unwrap_err()),
        format!("{}", ContainerError::NotRegistered(id2.clone()))
    );
    assert_eq!(
        format!("{}", containers_manager.try_write(id2.clone()).unwrap_err()),
        format!("{}", ContainerError::NotRegistered(id2.clone()))
    );
    assert_eq!(
        format!(
            "{}",
            containers_manager.try_release(id2.clone()).unwrap_err()
        ),
        format!("{}", ContainerError::NotRegistered(id2.clone()))
    );
    assert_eq!(
        format!("{}", containers_manager.try_read(id3.clone()).unwrap_err()),
        format!("{}", ContainerError::NotRegistered(id3.clone()))
    );
    assert_eq!(
        format!("{}", containers_manager.try_write(id3.clone()).unwrap_err()),
        format!("{}", ContainerError::NotRegistered(id3.clone()))
    );
    assert_eq!(
        format!(
            "{}",
            containers_manager.try_release(id3.clone()).unwrap_err()
        ),
        format!("{}", ContainerError::NotRegistered(id3.clone()))
    );
}

#[test]
fn container_error_display_not_reserved() {
    let mut containers_manager = ContainersManager::default();
    let id1 = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::Stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    let id3 = Identifier::Process(1, 1);

    assert_eq!(containers_manager.register(id1.clone()), true);
    assert_eq!(containers_manager.register(id2.clone()), true);
    assert_eq!(containers_manager.register(id3.clone()), true);

    assert_eq!(
        format!(
            "{}",
            containers_manager.try_release(id1.clone()).unwrap_err()
        ),
        format!("{}", ContainerError::NotReserved(id1.clone()))
    );
    assert_eq!(
        format!(
            "{}",
            containers_manager.try_release(id2.clone()).unwrap_err()
        ),
        format!("{}", ContainerError::NotReserved(id2.clone()))
    );
    assert_eq!(
        format!(
            "{}",
            containers_manager.try_release(id3.clone()).unwrap_err()
        ),
        format!("{}", ContainerError::NotReserved(id3.clone()))
    );
}

#[test]
fn container_result_partial_eq() {
    let done1 = ContainerResult::Done;
    let done2 = ContainerResult::Done;
    let (_, rx1) = oneshot::channel();
    let wait1 = ContainerResult::Wait(rx1);
    let (_, rx2) = oneshot::channel();
    let wait2 = ContainerResult::Wait(rx2);
    let id1 = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::File("/test/path/file2.txt".to_string());
    let error_not_registered1_id1 =
        ContainerResult::Error(ContainerError::NotRegistered(id1.clone()));
    let error_not_registered2_id1 =
        ContainerResult::Error(ContainerError::NotRegistered(id1.clone()));
    let error_not_registered1_id2 =
        ContainerResult::Error(ContainerError::NotRegistered(id2.clone()));
    let error_not_registered2_id2 =
        ContainerResult::Error(ContainerError::NotRegistered(id2.clone()));
    let error_not_reserved1_id1 = ContainerResult::Error(ContainerError::NotReserved(id1.clone()));
    let error_not_reserved2_id1 = ContainerResult::Error(ContainerError::NotReserved(id1.clone()));
    let error_not_reserved1_id2 = ContainerResult::Error(ContainerError::NotReserved(id2.clone()));
    let error_not_reserved2_id2 = ContainerResult::Error(ContainerError::NotReserved(id2.clone()));

    assert_eq!(done1, done2);
    assert_eq!(wait1, wait2);
    assert_eq!(error_not_registered1_id1, error_not_registered2_id1);
    assert_eq!(error_not_registered1_id2, error_not_registered2_id2);
    assert_eq!(error_not_reserved1_id1, error_not_reserved2_id1);
    assert_eq!(error_not_reserved1_id2, error_not_reserved2_id2);

    assert_ne!(done1, wait1);
    assert_ne!(done1, error_not_registered1_id1);
    assert_ne!(done1, error_not_reserved1_id1);

    assert_ne!(wait1, done1);
    assert_ne!(wait1, error_not_registered1_id1);
    assert_ne!(wait1, error_not_reserved1_id1);

    assert_ne!(error_not_registered1_id1, done1);
    assert_ne!(error_not_registered1_id1, wait1);
    assert_ne!(error_not_registered1_id1, error_not_reserved1_id1);

    assert_ne!(error_not_reserved1_id1, done1);
    assert_ne!(error_not_reserved1_id1, wait1);
    assert_ne!(error_not_reserved1_id1, error_not_registered1_id1);

    assert_ne!(error_not_registered1_id1, error_not_registered1_id2);
    assert_ne!(error_not_reserved1_id1, error_not_reserved1_id2);
}

#[tokio::test]
async fn containers_manager_action_register() -> Result<(), Box<dyn std::error::Error>> {
    let (sender, receiver) = mpsc::channel(32);

    tokio::spawn(containers_manager(receiver));

    let id1 = Identifier::File("/test/path/file1.txt".to_string());
    let (tx, rx) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::Register(id1.clone(), tx))
        .await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    let (tx, rx) = oneshot::channel();
    let _ = sender.send(ContainerAction::Register(id1, tx)).await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    Ok(())
}

#[tokio::test]
async fn containers_manager_action_reserve_no_queue_read_write(
) -> Result<(), Box<dyn std::error::Error>> {
    let (sender, receiver) = mpsc::channel(32);

    tokio::spawn(containers_manager(receiver));

    let id1 = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::File("/test/path/file2.txt".to_string());

    let (tx, rx) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveRead(id1.clone(), tx))
        .await;
    assert_eq!(
        rx.await.unwrap(),
        ContainerResult::Error(ContainerError::NotRegistered(id1.clone()))
    );

    let (tx, rx) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveWrite(id2.clone(), tx))
        .await;
    assert_eq!(
        rx.await.unwrap(),
        ContainerResult::Error(ContainerError::NotRegistered(id2.clone()))
    );

    let (tx, rx) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::Register(id1.clone(), tx))
        .await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    let (tx, rx) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::Register(id2.clone(), tx))
        .await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    let (tx, rx) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveRead(id1.clone(), tx))
        .await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    let (tx, rx) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveWrite(id2.clone(), tx))
        .await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    Ok(())
}

#[tokio::test]
async fn containers_manager_action_reserve_no_queue_multi_read(
) -> Result<(), Box<dyn std::error::Error>> {
    let (sender, receiver) = mpsc::channel(32);

    tokio::spawn(containers_manager(receiver));

    let id1 = Identifier::File("/test/path/file1.txt".to_string());

    let (tx, rx) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::Register(id1.clone(), tx))
        .await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    let (tx, rx) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveRead(id1.clone(), tx))
        .await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    let (tx, rx) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveRead(id1.clone(), tx))
        .await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    let (tx, rx) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveRead(id1.clone(), tx))
        .await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    Ok(())
}

#[tokio::test]
async fn containers_manager_action_reserve_queue_multi_write(
) -> Result<(), Box<dyn std::error::Error>> {
    let (sender, receiver) = mpsc::channel(32);

    tokio::spawn(containers_manager(receiver));

    let id1 = Identifier::File("/test/path/file1.txt".to_string());

    let (tx, rx) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::Register(id1.clone(), tx))
        .await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    let (tx, rx) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveWrite(id1.clone(), tx))
        .await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    let (tx, rx) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveWrite(id1.clone(), tx))
        .await;
    assert_eq!(
        rx.await.unwrap(),
        ContainerResult::Wait(oneshot::channel().1)
    );

    let (tx, rx) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveWrite(id1.clone(), tx))
        .await;
    assert_eq!(
        rx.await.unwrap(),
        ContainerResult::Wait(oneshot::channel().1)
    );

    Ok(())
}

#[tokio::test]
async fn containers_manager_action_reserve_queue_read_write(
) -> Result<(), Box<dyn std::error::Error>> {
    let (sender, receiver) = mpsc::channel(32);

    tokio::spawn(containers_manager(receiver));

    let id1 = Identifier::File("/test/path/file1.txt".to_string());

    let (tx, rx) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::Register(id1.clone(), tx))
        .await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    let (tx, rx) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveWrite(id1.clone(), tx))
        .await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    let (tx, rx) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveRead(id1.clone(), tx))
        .await;
    assert_eq!(
        rx.await.unwrap(),
        ContainerResult::Wait(oneshot::channel().1)
    );

    let (tx, rx) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveWrite(id1.clone(), tx))
        .await;
    assert_eq!(
        rx.await.unwrap(),
        ContainerResult::Wait(oneshot::channel().1)
    );

    Ok(())
}

#[tokio::test]
async fn containers_manager_action_release() -> Result<(), Box<dyn std::error::Error>> {
    let (sender, receiver) = mpsc::channel(32);

    tokio::spawn(containers_manager(receiver));

    let id1 = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::File("/test/path/file2.txt".to_string());

    let (tx, rx) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::Register(id1.clone(), tx))
        .await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    let (tx, rx) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::Register(id2.clone(), tx))
        .await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    let (tx, rx) = oneshot::channel();
    let _ = sender.send(ContainerAction::Release(id1.clone(), tx)).await;
    assert_eq!(
        rx.await.unwrap(),
        ContainerResult::Error(ContainerError::NotReserved(id1.clone()))
    );

    let (tx, rx) = oneshot::channel();
    let _ = sender.send(ContainerAction::Release(id2.clone(), tx)).await;
    assert_eq!(
        rx.await.unwrap(),
        ContainerResult::Error(ContainerError::NotReserved(id2.clone()))
    );

    let (tx, rx) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveRead(id1.clone(), tx))
        .await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    let (tx, rx) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveWrite(id2.clone(), tx))
        .await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    let (tx, rx) = oneshot::channel();
    let _ = sender.send(ContainerAction::Release(id1.clone(), tx)).await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    let (tx, rx) = oneshot::channel();
    let _ = sender.send(ContainerAction::Release(id2.clone(), tx)).await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    Ok(())
}

#[tokio::test]
async fn containers_manager_action_reserve_wait_release() -> Result<(), RecvError> {
    let (sender, receiver) = mpsc::channel(32);

    tokio::spawn(containers_manager(receiver));

    let id1 = Identifier::File("/test/path/file1.txt".to_string());

    let (tx, rx) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::Register(id1.clone(), tx))
        .await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    let (tx, rx) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveWrite(id1.clone(), tx))
        .await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    let (tx, rx) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveRead(id1.clone(), tx))
        .await;
    let callback = match rx.await.unwrap() {
        ContainerResult::Wait(callback) => callback,
        _ => panic!(),
    };

    let (tx1, rx1) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::Release(id1.clone(), tx1))
        .await;
    assert_eq!(rx1.await.unwrap(), ContainerResult::Done);

    callback.await
}
