use super::*;

#[test]
fn containers_manager_register() {
    let mut containers_manager = ContainersManager::default();
    let id1 = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::File("/test/path/file2.txt".to_string());
    assert_eq!(containers_manager.register(id1.clone()), true);
    assert_eq!(containers_manager.register(id2.clone()), true);
    assert_eq!(containers_manager.register(id1.clone()), false);
}

#[test]
fn containers_manager_try_reservation() {
    let mut containers_manager = ContainersManager::default();
    let id1 = Identifier::File("/test/path/file1.txt".to_string());
    let id2 = Identifier::File("/test/path/file2.txt".to_string());
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
