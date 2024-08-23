use tokio::{
    sync::{mpsc, oneshot},
    time::{timeout, Duration},
};

use crate::identifiers::Identifier;

use super::*;

#[test]
fn container_reservation_result_partial_eq() {
    let wait1 = ContainerReservationResult::Wait(oneshot::channel().1);
    let wait2 = ContainerReservationResult::Wait(oneshot::channel().1);
    let id1 = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::File("/test/path/file2.txt".to_string());
    let error_not_registered1_id1 =
        ContainerReservationResult::Error(ContainerError::NotRegistered(id1.clone()));
    let error_not_registered2_id1 =
        ContainerReservationResult::Error(ContainerError::NotRegistered(id1.clone()));
    let error_not_registered1_id2 =
        ContainerReservationResult::Error(ContainerError::NotRegistered(id2.clone()));
    let error_not_registered2_id2 =
        ContainerReservationResult::Error(ContainerError::NotRegistered(id2.clone()));
    let error_not_reserved1_id1 =
        ContainerReservationResult::Error(ContainerError::NotReserved(id1.clone()));
    let error_not_reserved2_id1 =
        ContainerReservationResult::Error(ContainerError::NotReserved(id1.clone()));
    let error_not_reserved1_id2 =
        ContainerReservationResult::Error(ContainerError::NotReserved(id2.clone()));
    let error_not_reserved2_id2 =
        ContainerReservationResult::Error(ContainerError::NotReserved(id2.clone()));

    assert_eq!(wait1, wait2);
    assert_eq!(error_not_registered1_id1, error_not_registered2_id1);
    assert_eq!(error_not_registered1_id2, error_not_registered2_id2);
    assert_eq!(error_not_reserved1_id1, error_not_reserved2_id1);
    assert_eq!(error_not_reserved1_id2, error_not_reserved2_id2);

    assert_ne!(wait1, error_not_registered1_id1);
    assert_ne!(wait1, error_not_reserved1_id1);

    assert_ne!(error_not_registered1_id1, wait1);
    assert_ne!(error_not_registered1_id1, error_not_reserved1_id1);

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
    let (tx1, rx1) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::Register(id1.clone(), tx1))
        .await;
    assert_eq!(rx1.await.unwrap(), ContainerResult::Done);

    let (tx2, rx2) = oneshot::channel();
    let _ = sender.send(ContainerAction::Register(id1, tx2)).await;
    assert_eq!(rx2.await.unwrap(), ContainerResult::Done);

    Ok(())
}

#[tokio::test]
async fn containers_manager_action_reserve_read() {
    let (sender, receiver) = mpsc::channel(32);
    tokio::spawn(containers_manager(receiver));
    let id1 = Identifier::File("/test/path/file1.txt".to_string());

    let (tx1, rx1) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::Register(id1.clone(), tx1))
        .await;
    assert_eq!(rx1.await.unwrap(), ContainerResult::Done);

    let (tx2, rx2) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveRead(id1.clone(), tx2))
        .await;
    let read1_reservation_status = rx2.await.unwrap();
    match read1_reservation_status {
        ContainerReservationResult::Wait(result) => result.await.unwrap(),
        _ => panic!(),
    };
}

#[tokio::test]
async fn containers_manager_action_reserve_read_not_registered() {
    let (sender, receiver) = mpsc::channel(32);
    tokio::spawn(containers_manager(receiver));
    let id1 = Identifier::File("/test/path/file1.txt".to_string());

    let (tx1, rx1) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveRead(id1.clone(), tx1))
        .await;

    match rx1.await.unwrap() {
        ContainerReservationResult::Error(e) => assert_eq!(
            format!("{}", e),
            format!("{}", ContainerError::NotRegistered(id1))
        ),
        _ => panic!(),
    }
}

#[tokio::test]
#[should_panic]
async fn containers_manager_action_reserve_read_timeout() {
    let (sender, receiver) = mpsc::channel(32);
    tokio::spawn(containers_manager(receiver));
    let id1 = Identifier::File("/test/path/file1.txt".to_string());

    let (tx, rx) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::Register(id1.clone(), tx))
        .await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    let (tx1, rx1) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveWrite(id1.clone(), tx1))
        .await;
    match rx1.await.unwrap() {
        ContainerReservationResult::Wait(result) => result.await.unwrap(),
        _ => (),
    };

    let (tx2, rx2) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveRead(id1.clone(), tx2))
        .await;
    match rx2.await.unwrap() {
        ContainerReservationResult::Wait(result) => {
            assert!(timeout(Duration::from_millis(100), result).await.is_err())
        }
        _ => (),
    };
}

#[tokio::test]
async fn containers_manager_action_reserve_write() {
    let (sender, receiver) = mpsc::channel(32);
    tokio::spawn(containers_manager(receiver));
    let id1 = Identifier::File("/test/path/file1.txt".to_string());

    let (tx1, rx1) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::Register(id1.clone(), tx1))
        .await;
    assert_eq!(rx1.await.unwrap(), ContainerResult::Done);

    let (tx2, rx2) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveWrite(id1.clone(), tx2))
        .await;
    let read1_reservation_status = rx2.await.unwrap();
    match read1_reservation_status {
        ContainerReservationResult::Wait(result) => result.await.unwrap(),
        _ => panic!(),
    };
}

#[tokio::test]
async fn containers_manager_action_reserve_write_not_registered() {
    let (sender, receiver) = mpsc::channel(32);
    tokio::spawn(containers_manager(receiver));
    let id1 = Identifier::File("/test/path/file1.txt".to_string());

    let (tx1, rx1) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveWrite(id1.clone(), tx1))
        .await;

    match rx1.await.unwrap() {
        ContainerReservationResult::Error(e) => assert_eq!(
            format!("{}", e),
            format!("{}", ContainerError::NotRegistered(id1))
        ),
        _ => panic!(),
    }
}

#[tokio::test]
#[should_panic]
async fn containers_manager_action_reserve_write_timeout() {
    let (sender, receiver) = mpsc::channel(32);
    tokio::spawn(containers_manager(receiver));
    let id1 = Identifier::File("/test/path/file1.txt".to_string());

    let (tx, rx) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::Register(id1.clone(), tx))
        .await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    let (tx1, rx1) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveRead(id1.clone(), tx1))
        .await;
    match rx1.await.unwrap() {
        ContainerReservationResult::Wait(result) => result.await.unwrap(),
        _ => (),
    };

    let (tx2, rx2) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveWrite(id1.clone(), tx2))
        .await;
    match rx2.await.unwrap() {
        ContainerReservationResult::Wait(result) => {
            assert!(timeout(Duration::from_millis(100), result).await.is_err())
        }
        _ => (),
    };
}

#[tokio::test]
#[should_panic]
async fn containers_manager_action_reserve_2write_timeout() {
    let (sender, receiver) = mpsc::channel(32);
    tokio::spawn(containers_manager(receiver));
    let id1 = Identifier::File("/test/path/file1.txt".to_string());

    let (tx, rx) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::Register(id1.clone(), tx))
        .await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    let (tx1, rx1) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveWrite(id1.clone(), tx1))
        .await;
    match rx1.await.unwrap() {
        ContainerReservationResult::Wait(result) => result.await.unwrap(),
        _ => (),
    };

    let (tx2, rx2) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveWrite(id1.clone(), tx2))
        .await;
    match rx2.await.unwrap() {
        ContainerReservationResult::Wait(result) => {
            assert!(timeout(Duration::from_millis(100), result).await.is_err())
        }
        _ => (),
    };
}

#[tokio::test]
async fn containers_manager_action_release_not_reserved() {
    let (sender, receiver) = mpsc::channel(32);
    tokio::spawn(containers_manager(receiver));
    let id1 = Identifier::File("/test/path/file1.txt".to_string());

    let (tx1, rx1) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::Register(id1.clone(), tx1))
        .await;
    assert_eq!(rx1.await.unwrap(), ContainerResult::Done);

    let (tx2, rx2) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::Release(id1.clone(), tx2))
        .await;

    match rx2.await.unwrap() {
        ContainerResult::Error(e) => assert_eq!(
            format!("{}", e),
            format!("{}", ContainerError::NotReserved(id1))
        ),
        _ => panic!(),
    }
}

#[tokio::test]
async fn containers_manager_action_release_not_registered() {
    let (sender, receiver) = mpsc::channel(32);
    tokio::spawn(containers_manager(receiver));
    let id1 = Identifier::File("/test/path/file1.txt".to_string());

    let (tx1, rx1) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::Release(id1.clone(), tx1))
        .await;
    assert_eq!(
        format!("{:?}", rx1.await.unwrap()),
        format!(
            "{:?}",
            ContainerReservationResult::Error(ContainerError::NotRegistered(id1))
        )
    );
}

#[tokio::test]
async fn containers_manager_action_release_read() {
    let (sender, receiver) = mpsc::channel(32);
    tokio::spawn(containers_manager(receiver));
    let id1 = Identifier::File("/test/path/file1.txt".to_string());

    let (tx1, rx1) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::Register(id1.clone(), tx1))
        .await;
    assert_eq!(rx1.await.unwrap(), ContainerResult::Done);

    let (tx2, rx2) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveRead(id1.clone(), tx2))
        .await;
    match rx2.await.unwrap() {
        ContainerReservationResult::Wait(result) => result.await.unwrap(),
        _ => panic!(),
    };

    let (tx3, rx3) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::Release(id1.clone(), tx3))
        .await;
    assert_eq!(rx3.await.unwrap(), ContainerResult::Done);
}

#[tokio::test]
async fn containers_manager_action_release_write() {
    let (sender, receiver) = mpsc::channel(32);
    tokio::spawn(containers_manager(receiver));
    let id1 = Identifier::File("/test/path/file1.txt".to_string());

    let (tx1, rx1) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::Register(id1.clone(), tx1))
        .await;
    assert_eq!(rx1.await.unwrap(), ContainerResult::Done);

    let (tx2, rx2) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveWrite(id1.clone(), tx2))
        .await;
    match rx2.await.unwrap() {
        ContainerReservationResult::Wait(result) => result.await.unwrap(),
        _ => panic!(),
    };

    let (tx3, rx3) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::Release(id1.clone(), tx3))
        .await;
    assert_eq!(rx3.await.unwrap(), ContainerResult::Done);
}

#[tokio::test]
async fn containers_manager_action_reserve_fair() {
    let (sender, receiver) = mpsc::channel(32);
    tokio::spawn(containers_manager(receiver));
    let id1 = Identifier::File("/test/path/file1.txt".to_string());

    let (tx, rx) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::Register(id1.clone(), tx))
        .await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    let (tx1, rx1) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveRead(id1.clone(), tx1))
        .await;
    match rx1.await.unwrap() {
        ContainerReservationResult::Wait(result) => result.await.unwrap(),
        _ => panic!(),
    };

    let (tx2, rx2) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveRead(id1.clone(), tx2))
        .await;
    match rx2.await.unwrap() {
        ContainerReservationResult::Wait(result) => result.await.unwrap(),
        _ => panic!(),
    };

    let (tx3, rx3) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveRead(id1.clone(), tx3))
        .await;
    match rx3.await.unwrap() {
        ContainerReservationResult::Wait(result) => result.await.unwrap(),
        _ => panic!(),
    };

    // 3 Read reservations to release to reserve Write
    let (tx, rx) = oneshot::channel();
    let _ = sender.send(ContainerAction::Release(id1.clone(), tx)).await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    let (tx, rx) = oneshot::channel();
    let _ = sender.send(ContainerAction::Release(id1.clone(), tx)).await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    let (tx, rx) = oneshot::channel();
    let _ = sender.send(ContainerAction::Release(id1.clone(), tx)).await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    let (tx4, rx4) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveWrite(id1.clone(), tx4))
        .await;
    match rx4.await.unwrap() {
        ContainerReservationResult::Wait(result) => result.await.unwrap(),
        _ => panic!(),
    };
    // Release first Write reservation to allow the next one
    let (tx, rx) = oneshot::channel();
    let _ = sender.send(ContainerAction::Release(id1.clone(), tx)).await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    let (tx5, rx5) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveWrite(id1.clone(), tx5))
        .await;
    match rx5.await.unwrap() {
        ContainerReservationResult::Wait(result) => result.await.unwrap(),
        _ => panic!(),
    };

    // Release last Write reservation to allow the next reservations
    let (tx, rx) = oneshot::channel();
    let _ = sender.send(ContainerAction::Release(id1.clone(), tx)).await;
    assert_eq!(rx.await.unwrap(), ContainerResult::Done);

    let (tx6, rx6) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveRead(id1.clone(), tx6))
        .await;
    match rx6.await.unwrap() {
        ContainerReservationResult::Wait(result) => result.await.unwrap(),
        _ => panic!(),
    };

    // No need to release Read when there are only Read reservations

    let (tx7, rx7) = oneshot::channel();
    let _ = sender
        .send(ContainerAction::ReserveRead(id1.clone(), tx7))
        .await;
    match rx7.await.unwrap() {
        ContainerReservationResult::Wait(result) => result.await.unwrap(),
        _ => panic!(),
    };
}
