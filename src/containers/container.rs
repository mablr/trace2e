/// Foundation object for traceability.
///
/// An instance of a [`Container`] holds all information necessary to provide traceability
/// features for a system resource designated by file descriptor, such as consistent
/// IO ordering management, provenance recording and compliance enforcement.  
#[derive(Debug)]
pub struct Container {
    read_count: usize,  // Number of readers holding the container
    write_locked: bool, // Indicates if the container is write-locked
}

impl Container {
    /// Create a new container
    pub fn new() -> Self {
        Self {
            read_count: 0,
            write_locked: false,
        }
    }

    pub fn is_read(&self) -> bool {
        self.read_count != 0
    }

    pub fn is_write(&self) -> bool {
        self.write_locked
    }

    /// Reserve the container for reading
    pub fn reserve_read(&mut self) -> bool {
        if !self.write_locked {
            self.read_count += 1;
            true
        } else {
            false
        }
    }

    /// Reserve the container for writing
    pub fn reserve_write(&mut self) -> bool {
        if self.read_count == 0 && !self.write_locked {
            self.write_locked = true;
            true
        } else {
            false
        }
    }

    /// Release a read reservation
    pub fn release_read(&mut self) -> bool {
        if self.read_count > 0 {
            self.read_count -= 1;
            true
        } else {
            false
        }
    }

    /// Release a write reservation
    pub fn release_write(&mut self) -> bool {
        if self.write_locked {
            self.write_locked = false;
            true
        } else {
            false
        }
    }
}
