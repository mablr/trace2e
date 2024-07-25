#[derive(Debug)]
pub struct ContainerManager {
    available: bool,
}

impl ContainerManager {
    pub fn default() -> Self {
        ContainerManager {
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