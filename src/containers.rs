use std::collections::HashMap;


/// Foundation object for traceability.
/// 
/// An instance of a [`Container`] holds all information necessary to provide traceability
/// features for a system resource designated by file descriptor, such as consistent 
/// IO ordering management, provenance recording and compliance enforcement.  
#[derive(Debug)]
pub struct Container {
    available: bool,
}

impl Container {
    fn new() -> Self {
        Container {
            available: true,
        }
    }

    fn is_available(&self) -> bool{
        self.available
    }

    fn set_availability(&mut self, state: bool) {
        self.available = state;
    }

}

/// Global management structure for [`Container`] instances.
/// 
/// It offers a reliable and safe interface to acquire reservation in order to manipulate 
/// `Containers`.
#[derive(Debug, Default)]
pub struct ContainersManager {
    containers: HashMap<String, Container>
}

impl ContainersManager {
    /// This method checks the presence of the provided key before instantiating 
    /// and inserting a new [`Container`] to avoid overwriting an existing [`Container`]
    /// 
    /// This will return `true` if a new [`Container`] has been instantiated and inserted,
    /// and `false` if a [`Container`] already exists for the provided key. 
    pub fn register(&mut self, resource_identifier: String) -> bool {
        if self.containers.contains_key(&resource_identifier) == false {
            let container = Container::new();
            self.containers.insert(resource_identifier, container);
            true
        } else {
            false
        }
    }

    /// Reserve the [`Container`] registered with the provided key.
    /// 
    /// If the [`Container`] is available, it is set as reserved and `Ok(true)` is 
    /// returned, if the [`Container`] is already reserved `Ok(false)` is returned.
    /// 
    /// # Errors
    /// If there is no [`Container`] registered with the provided key an error is returned.
    pub fn try_reservation(&mut self, resource_identifier: String) -> Result<bool, String> {        
        if let Some(container) = self.containers.get_mut(&resource_identifier) {
            if container.is_available() {
                container.set_availability(false);
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            // Todo: Create specific error type
            Err(format!("Container '{}' is not registered, impossible to reserve it.", resource_identifier))
        }
    }

    /// Release the [`Container`] registered with the provided key.
    /// 
    /// If the [`Container`] is reserved, it is set as available and `Ok(())` is 
    /// returned
    /// 
    /// # Errors
    /// If the [`Container`] is already available an error is returned.
    /// 
    /// If there is no [`Container`] registered with the provided key an error is returned.
    pub fn try_release(&mut self, resource_identifier: String) -> Result<(), String> {        
        if let Some(container) = self.containers.get_mut(&resource_identifier) {
            if  container.is_available() == false {
                container.set_availability(true);
                Ok(())
            } else {
                // Todo: Create specific error type
                Err(format!("Container '{}' is not reserved, impossible to release it.", resource_identifier))
            }
        } else {
            // Todo: Create specific error type
            Err(format!("Container '{}' is not registered, impossible to release it.", resource_identifier))
        }
    }
}

#[cfg(test)]
mod tests {
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
}