use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use manager::ContainerError;

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
    let id3 = Identifier::Process(1);

    assert_eq!(containers_manager.register(id1.clone()), true);
    assert_eq!(containers_manager.register(id2.clone()), true);
    assert_eq!(containers_manager.register(id1.clone()), false);
    assert_eq!(containers_manager.register(id3.clone()), true);
    assert_eq!(containers_manager.register(id3.clone()), false);
    assert_eq!(containers_manager.register(id2.clone()), false);
    assert_eq!(containers_manager.register(id1.clone()), false);
}

#[test]
fn containers_manager_try_read() {
    let mut containers_manager = ContainersManager::default();
    let id1 = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::Stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    let id3 = Identifier::Process(1);

    assert_eq!(containers_manager.register(id1.clone()), true);
    assert_eq!(containers_manager.register(id2.clone()), true);
    assert_eq!(containers_manager.register(id3.clone()), true);

    assert_eq!(containers_manager.try_read(id1.clone()), Ok(()));
    assert_eq!(containers_manager.try_read(id2.clone()), Ok(()));
    assert_eq!(containers_manager.try_read(id3.clone()), Ok(()));
}

#[test]
fn containers_manager_try_read_unregistered() {
    let mut containers_manager = ContainersManager::default();
    let id1: Identifier = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::Stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    let id3: Identifier = Identifier::Process(1);

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
    let id1: Identifier = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::Stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    let id3: Identifier = Identifier::Process(1);

    assert_eq!(containers_manager.register(id1.clone()), true);
    assert_eq!(containers_manager.register(id2.clone()), true);
    assert_eq!(containers_manager.register(id3.clone()), true);

    assert_eq!(containers_manager.try_read(id1.clone()), Ok(()));
    assert_eq!(containers_manager.try_read(id2.clone()), Ok(()));
    assert_eq!(containers_manager.try_read(id3.clone()), Ok(()));

    assert_eq!(containers_manager.try_read(id1.clone()), Ok(()));
    assert_eq!(containers_manager.try_read(id2.clone()), Ok(()));
    assert_eq!(containers_manager.try_read(id3.clone()), Ok(()));
}

#[test]
fn containers_manager_try_read_already_write() {
    let mut containers_manager = ContainersManager::default();
    let id1: Identifier = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::Stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    let id3: Identifier = Identifier::Process(1);

    assert_eq!(containers_manager.register(id1.clone()), true);
    assert_eq!(containers_manager.register(id2.clone()), true);
    assert_eq!(containers_manager.register(id3.clone()), true);

    assert_eq!(containers_manager.try_write(id1.clone()), Ok(()));
    assert_eq!(containers_manager.try_write(id2.clone()), Ok(()));
    assert_eq!(containers_manager.try_write(id3.clone()), Ok(()));

    assert_eq!(
        containers_manager.try_read(id1.clone()),
        Err(ContainerError::AlreadyReserved(id1.clone()))
    );
    assert_eq!(
        containers_manager.try_read(id2.clone()),
        Err(ContainerError::AlreadyReserved(id2.clone()))
    );
    assert_eq!(
        containers_manager.try_read(id3.clone()),
        Err(ContainerError::AlreadyReserved(id3.clone()))
    );
}

#[test]
fn containers_manager_try_write() {
    let mut containers_manager = ContainersManager::default();
    let id1: Identifier = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::Stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    let id3: Identifier = Identifier::Process(1);

    assert_eq!(containers_manager.register(id1.clone()), true);
    assert_eq!(containers_manager.register(id2.clone()), true);
    assert_eq!(containers_manager.register(id3.clone()), true);

    assert_eq!(containers_manager.try_write(id1.clone()), Ok(()));
    assert_eq!(containers_manager.try_write(id2.clone()), Ok(()));
    assert_eq!(containers_manager.try_write(id3.clone()), Ok(()));
}

#[test]
fn containers_manager_try_write_unregistered() {
    let mut containers_manager = ContainersManager::default();
    let id1: Identifier = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::Stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    let id3: Identifier = Identifier::Process(1);

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
    let id1: Identifier = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::Stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    let id3: Identifier = Identifier::Process(1);

    assert_eq!(containers_manager.register(id1.clone()), true);
    assert_eq!(containers_manager.register(id2.clone()), true);
    assert_eq!(containers_manager.register(id3.clone()), true);

    assert_eq!(containers_manager.try_read(id1.clone()), Ok(()));
    assert_eq!(containers_manager.try_read(id2.clone()), Ok(()));
    assert_eq!(containers_manager.try_read(id3.clone()), Ok(()));

    assert_eq!(
        containers_manager.try_write(id1.clone()),
        Err(ContainerError::AlreadyReserved(id1.clone()))
    );
    assert_eq!(
        containers_manager.try_write(id2.clone()),
        Err(ContainerError::AlreadyReserved(id2.clone()))
    );
    assert_eq!(
        containers_manager.try_write(id3.clone()),
        Err(ContainerError::AlreadyReserved(id3.clone()))
    );
}

#[test]
fn containers_manager_try_write_already_write() {
    let mut containers_manager = ContainersManager::default();
    let id1: Identifier = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::Stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    let id3: Identifier = Identifier::Process(1);

    assert_eq!(containers_manager.register(id1.clone()), true);
    assert_eq!(containers_manager.register(id2.clone()), true);
    assert_eq!(containers_manager.register(id3.clone()), true);

    assert_eq!(containers_manager.try_write(id1.clone()), Ok(()));
    assert_eq!(containers_manager.try_write(id2.clone()), Ok(()));
    assert_eq!(containers_manager.try_write(id3.clone()), Ok(()));

    assert_eq!(
        containers_manager.try_write(id1.clone()),
        Err(ContainerError::AlreadyReserved(id1.clone()))
    );
    assert_eq!(
        containers_manager.try_write(id2.clone()),
        Err(ContainerError::AlreadyReserved(id2.clone()))
    );
    assert_eq!(
        containers_manager.try_write(id3.clone()),
        Err(ContainerError::AlreadyReserved(id3.clone()))
    );
}

#[test]
fn containers_manager_try_release_write() {
    let mut containers_manager = ContainersManager::default();
    let id1: Identifier = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::Stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    let id3: Identifier = Identifier::Process(1);

    assert_eq!(containers_manager.register(id1.clone()), true);
    assert_eq!(containers_manager.register(id2.clone()), true);
    assert_eq!(containers_manager.register(id3.clone()), true);

    assert_eq!(containers_manager.try_write(id1.clone()), Ok(()));
    assert_eq!(containers_manager.try_write(id2.clone()), Ok(()));
    assert_eq!(containers_manager.try_write(id3.clone()), Ok(()));

    assert_eq!(containers_manager.try_release(id1.clone()), Ok(()));
    assert_eq!(containers_manager.try_release(id2.clone()), Ok(()));
    assert_eq!(containers_manager.try_release(id3.clone()), Ok(()));

    assert_eq!(containers_manager.try_write(id1.clone()), Ok(()));
    assert_eq!(containers_manager.try_write(id2.clone()), Ok(()));
    assert_eq!(containers_manager.try_write(id3.clone()), Ok(()));
}

#[test]
fn containers_manager_try_release_read() {
    let mut containers_manager = ContainersManager::default();
    let id1: Identifier = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::Stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    let id3: Identifier = Identifier::Process(1);

    assert_eq!(containers_manager.register(id1.clone()), true);
    assert_eq!(containers_manager.register(id2.clone()), true);
    assert_eq!(containers_manager.register(id3.clone()), true);

    assert_eq!(containers_manager.try_read(id1.clone()), Ok(()));
    assert_eq!(containers_manager.try_read(id2.clone()), Ok(()));
    assert_eq!(containers_manager.try_read(id3.clone()), Ok(()));

    assert_eq!(containers_manager.try_release(id1.clone()), Ok(()));
    assert_eq!(containers_manager.try_release(id2.clone()), Ok(()));
    assert_eq!(containers_manager.try_release(id3.clone()), Ok(()));

    assert_eq!(containers_manager.try_write(id1.clone()), Ok(()));
    assert_eq!(containers_manager.try_write(id2.clone()), Ok(()));
    assert_eq!(containers_manager.try_write(id3.clone()), Ok(()));
}

#[test]
fn containers_manager_try_release_available() {
    let mut containers_manager = ContainersManager::default();
    let id1: Identifier = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::Stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    let id3: Identifier = Identifier::Process(1);

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
fn containers_manager_try_release_unregistered() {
    let mut containers_manager = ContainersManager::default();
    let id1: Identifier = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::Stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    let id3: Identifier = Identifier::Process(1);

    assert_eq!(
        containers_manager.try_release(id1.clone()),
        Err(ContainerError::NotRegistered(id1.clone()))
    );
    assert_eq!(
        containers_manager.try_release(id2.clone()),
        Err(ContainerError::NotRegistered(id2.clone()))
    );
    assert_eq!(
        containers_manager.try_release(id3.clone()),
        Err(ContainerError::NotRegistered(id3.clone()))
    );
}
