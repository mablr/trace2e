use std::collections::HashMap;

#[derive(Debug)]
pub struct Container {
    available: bool,
}

impl Container {
    pub fn default() -> Self {
        Container {
            available: true,
        }
    }

    pub fn is_available(&self) -> bool{
        self.available
    }

    pub fn set_availability(&mut self, state: bool) {
        self.available = state;
    }

}

#[derive(Debug, Default)]
pub struct ContainersManager {
    containers: HashMap<String, Container>
}

impl ContainersManager {
    pub fn register(&mut self, resource_identifier: String) {
        if self.containers.contains_key(&resource_identifier) == false {
            self.containers.insert(resource_identifier, Container::default());
        }
    }

    pub fn try_reservation(&mut self, resource_identifier: String) -> Result<bool, String> {        
        if let Some(container) = self.containers.get_mut(&resource_identifier) {
            if container.is_available() {
                container.set_availability(false);
                Ok(true)
            } else {
                // Already reserved, wait a bit
                Ok(false)
            }
        } else {
            // Todo: Create specific error type
            Err(format!("Container '{}' is not registered, impossible to reserve it.", resource_identifier))
        }
    }

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