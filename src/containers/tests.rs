use std::net::{IpAddr, Ipv4Addr, SocketAddr};

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
fn containers_manager_try_reservation() {
    let mut containers_manager = ContainersManager::default();
    let id1 = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::Stream(
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12312),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
    );
    assert_eq!(containers_manager.register(id1.clone()), true);
    assert_eq!(containers_manager.try_reservation(id1.clone()), Ok(true));
    assert_eq!(containers_manager.try_reservation(id1.clone()), Ok(false));
    assert_eq!(
        containers_manager.try_reservation(id2.clone()),
        Err(format!(
            "Container '{}' is not registered, impossible to reserve it.",
            id2.clone()
        ))
    );
}

#[test]
fn containers_manager_try_release() {
    let mut containers_manager = ContainersManager::default();
    let id1 = Identifier::File("/test/path/file1.txt".to_string());
    assert_eq!(
        containers_manager.try_release(id1.clone()),
        Err(format!(
            "Container '{}' is not registered, impossible to release it.",
            id1.clone()
        ))
    );
    assert_eq!(containers_manager.register(id1.clone()), true);
    assert_eq!(
        containers_manager.try_release(id1.clone()),
        Err(format!(
            "Container '{}' is not reserved, impossible to release it.",
            id1.clone()
        ))
    );
    assert_eq!(containers_manager.try_reservation(id1.clone()), Ok(true));
    assert_eq!(containers_manager.try_release(id1.clone()), Ok(()));
    assert_eq!(
        containers_manager.try_release(id1.clone()),
        Err(format!(
            "Container '{}' is not reserved, impossible to release it.",
            id1.clone()
        ))
    );
}
