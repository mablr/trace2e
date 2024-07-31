use super::*;

#[test]
fn containers_manager_register() {
    let mut containers_manager = ContainersManager::default();
    let path1 = "/test/path/file1.txt".to_string();
    let path2 = "/test/path/file2.txt".to_string();
    assert_eq!(containers_manager.register(path1.clone()), true);
    assert_eq!(containers_manager.register(path2.clone()), true);
    assert_eq!(containers_manager.register(path1.clone()), false);
}

#[test]
fn containers_manager_try_reservation() {
    let mut containers_manager = ContainersManager::default();
    let path1 = "/test/path/file1.txt".to_string();
    let path2 = "/test/path/file2.txt".to_string();
    assert_eq!(containers_manager.register(path1.clone()), true);
    assert_eq!(containers_manager.try_reservation(path1.clone()), Ok(true));
    assert_eq!(containers_manager.try_reservation(path1.clone()), Ok(false));
    assert_eq!(containers_manager.try_reservation(path2.clone()), 
        Err(format!("Container '{}' is not registered, impossible to reserve it.", path2.clone()))
    );
}

#[test]
fn containers_manager_try_release() {
    let mut containers_manager = ContainersManager::default();
    let path1 = "/test/path/file1.txt".to_string();
    assert_eq!(containers_manager.try_release(path1.clone()),
        Err(format!("Container '{}' is not registered, impossible to release it.", path1.clone()))
    );
    assert_eq!(containers_manager.register(path1.clone()), true);
    assert_eq!(containers_manager.try_release(path1.clone()),
        Err(format!("Container '{}' is not reserved, impossible to release it.", path1.clone()))
    );
    assert_eq!(containers_manager.try_reservation(path1.clone()), Ok(true));
    assert_eq!(containers_manager.try_release(path1.clone()), Ok(()));
    assert_eq!(containers_manager.try_release(path1.clone()),
        Err(format!("Container '{}' is not reserved, impossible to release it.", path1.clone()))
    );
}