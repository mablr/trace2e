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
    pub fn new() -> Self {
        Container { available: true }
    }

    pub fn is_available(&self) -> bool {
        self.available
    }

    pub fn set_availability(&mut self, state: bool) {
        self.available = state;
    }
}
