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