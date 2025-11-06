//! Common orchestration patterns for e2e demos
//!
//! This module provides helper structures for setting up resources,
//! enrolling files and streams, and performing I/O operations.
//! Macros are defined in the macros module.

use trace2e_core::traceability::infrastructure::naming::{Fd, Resource};

/// File mapping for demo resource management
#[derive(Clone)]
pub struct FileMapping {
    pid: i32,
    fd: i32,
    node_id: String,
    process: Resource,
    file: Resource,
}

impl FileMapping {
    pub fn new(pid: i32, fd: i32, path: &str, node_id: String) -> Self {
        Self {
            pid,
            fd,
            node_id,
            process: Resource::new_process(pid),
            file: Resource::new_file(path.to_string()),
        }
    }

    pub fn pid(&self) -> i32 {
        self.pid
    }

    pub fn fd(&self) -> i32 {
        self.fd
    }

    pub fn file_path(&self) -> String {
        match &self.file {
            Resource::Fd(Fd::File(file)) => file.path.to_owned(),
            _ => panic!("FileMapping is not a file"),
        }
    }

    pub fn process(&self) -> Resource {
        self.process.to_owned()
    }

    pub fn file(&self) -> Resource {
        self.file.to_owned()
    }

    pub fn node_id(&self) -> String {
        self.node_id.clone()
    }
}

/// Stream mapping for demo resource management
#[derive(Clone)]
pub struct StreamMapping {
    pid: i32,
    fd: i32,
    process: Resource,
    stream: Resource,
}

impl StreamMapping {
    pub fn new(pid: i32, fd: i32, local_socket: &str, peer_socket: &str) -> Self {
        Self {
            pid,
            fd,
            process: Resource::new_process(pid),
            stream: Resource::new_stream(local_socket.to_string(), peer_socket.to_string()),
        }
    }

    pub fn pid(&self) -> i32 {
        self.pid
    }

    pub fn fd(&self) -> i32 {
        self.fd
    }

    pub fn stream_local_socket(&self) -> String {
        match &self.stream {
            Resource::Fd(Fd::Stream(stream)) => stream.local_socket.to_owned(),
            _ => panic!("StreamMapping is not a stream"),
        }
    }

    pub fn stream_peer_socket(&self) -> String {
        match &self.stream {
            Resource::Fd(Fd::Stream(stream)) => stream.peer_socket.to_owned(),
            _ => panic!("StreamMapping is not a stream"),
        }
    }

    pub fn process(&self) -> Resource {
        self.process.to_owned()
    }

    pub fn stream(&self) -> Resource {
        self.stream.to_owned()
    }
}
