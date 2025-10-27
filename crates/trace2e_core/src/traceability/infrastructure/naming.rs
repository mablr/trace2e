//! Resource naming and identification system.
//!
//! This module defines the core resource identification and naming conventions used throughout
//! the trace2e traceability system. It provides a unified way to represent and identify
//! different types of computational resources including files, network streams, and processes.
//!
//! ## Resource Hierarchy
//!
//! **File Descriptors (Fd)**: Represent operating system file descriptors that can point to
//! either files in the filesystem or network streams (sockets).
//!
//! **Files**: Filesystem resources identified by their path, supporting both absolute and
//! relative path specifications.
//!
//! **Streams**: Network communication channels defined by local and peer socket addresses,
//! supporting TCP connections, Unix domain sockets, and other network protocols.
//!
//! **Processes**: Running system processes identified by PID with additional metadata including
//! start time and executable path for precise identification across process reuse.
//!
//! ## Resource Construction
//!
//! Resources can be constructed using dedicated factory methods that handle system queries
//! and validation, or using mock variants for testing purposes.

use std::fmt::Debug;

use sysinfo::{Pid, System};

/// Represents a file resource in the filesystem.
///
/// Files are identified by their path, which can be absolute or relative.
/// The path is stored as provided without normalization to preserve
/// the original specification for audit and debugging purposes.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct File {
    /// Filesystem path to the file, as specified by the application
    pub path: String,
}

/// Represents a network stream or socket connection.
///
/// Streams are bidirectional communication channels between two endpoints,
/// typically used for TCP connections, Unix domain sockets, or other
/// network protocols. Both endpoints must be specified to enable proper
/// flow tracking and policy enforcement.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Stream {
    /// Local socket address (e.g., "127.0.0.1:8080")
    pub local_socket: String,
    /// Remote peer socket address (e.g., "192.168.1.100:9000")
    pub peer_socket: String,
}

/// Represents a file descriptor that can point to either a file or a stream.
///
/// File descriptors are the operating system's handle for I/O operations.
/// This enum distinguishes between filesystem-based I/O (files) and
/// network-based I/O (streams) while maintaining a unified interface.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Fd {
    /// File descriptor pointing to a filesystem file
    File(File),
    /// File descriptor pointing to a network stream or socket
    Stream(Stream),
}

/// Represents a running system process with identifying metadata.
///
/// Processes are identified not only by their PID (which can be reused)
/// but also by their start time and executable path to ensure precise
/// identification across the system lifecycle.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Process {
    /// Process identifier assigned by the operating system
    pub pid: i32,
    /// Process start time in seconds since epoch for uniqueness
    pub starttime: u64,
    /// Path to the executable that created this process
    pub exe_path: String,
}

/// Unified resource identifier for all trackable entities in the system.
///
/// Resources represent any entity that can participate in data flows within
/// the traceability system. This includes file descriptors (which may point
/// to files or streams), processes, or null resources for uninitialized states.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Default)]
pub enum Resource {
    /// File descriptor resource (file or stream)
    Fd(Fd),
    /// Process resource  
    Process(Process),
    /// Null or uninitialized resource
    #[default]
    None,
}

impl Resource {
    /// Creates a new file resource with the specified filesystem path.
    ///
    /// The path is stored as provided without validation or normalization.
    /// Applications should provide valid paths to ensure proper resource tracking.
    pub fn new_file(path: String) -> Self {
        Self::Fd(Fd::File(File { path }))
    }

    /// Creates a new stream resource with the specified socket addresses.
    ///
    /// Both local and peer socket addresses should be valid network addresses
    /// in the format appropriate for the protocol (e.g., "IP:port" for TCP).
    pub fn new_stream(local_socket: String, peer_socket: String) -> Self {
        Self::Fd(Fd::Stream(Stream { local_socket, peer_socket }))
    }

    /// Creates a new process resource by querying the system for process information.
    ///
    /// Attempts to retrieve the process start time and executable path from the
    /// system process table. If the process is not found, creates a process
    /// resource with default values for the metadata fields.
    pub fn new_process(pid: i32) -> Self {
        let mut system = System::new();
        system.refresh_all();
        if let Some(process) = system.process(Pid::from(pid as usize)) {
            let starttime = process.start_time();
            let exe_path = if let Some(exe) = process.exe() {
                exe.to_string_lossy().to_string()
            } else {
                String::new()
            };
            Self::Process(Process { pid, starttime, exe_path })
        } else {
            Self::Process(Process { pid, starttime: 0, exe_path: String::new() })
        }
    }

    /// Creates a mock process resource for testing purposes.
    ///
    /// Creates a process resource with the specified PID but default values
    /// for start time and executable path. Should only be used in test
    /// environments where system process queries are not needed.
    pub fn new_process_mock(pid: i32) -> Self {
        Self::Process(Process { pid, starttime: 0, exe_path: String::new() })
    }

    /// Checks if this resource represents a filesystem file.
    ///
    /// Returns true if the resource is a file descriptor pointing to a file,
    /// false for streams, processes, or null resources.
    pub fn is_file(&self) -> bool {
        matches!(self, Resource::Fd(Fd::File(_)))
    }

    /// Returns the reverse stream resource if this is a stream resource.
    ///
    /// For stream resources, returns a new resource with the local and peer
    /// socket addresses swapped. This is useful for tracking bidirectional
    /// flows. Returns None for non-stream resources.
    pub fn is_stream(&self) -> Option<Self> {
        if let Resource::Fd(Fd::Stream(stream)) = self {
            Some(Self::new_stream(stream.peer_socket.to_owned(), stream.local_socket.to_owned()))
        } else {
            None
        }
    }

    /// Checks if this resource represents a system process.
    ///
    /// Returns true if the resource is a process, false for file descriptors
    /// or null resources.
    pub fn is_process(&self) -> bool {
        matches!(self, Resource::Process(_))
    }
}

/// Unified resource identifier for all trackable entities in the system.
///
/// Localized resources are resources that are associated with a specific node in a distributed system.
/// They are used to identify resources that are local to a specific node and are used to track
/// resources that are local to a specific node.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Default)]
pub struct LocalizedResource {
    /// Node identifier for this localized resource
    node_id: String,
    /// Resource for this localized resource
    resource: Resource,
}

impl LocalizedResource {
    /// Creates a new localized resource with the specified node identifier and resource.
    pub fn new(node_id: String, resource: Resource) -> Self {
        Self { node_id, resource }
    }

    /// Returns the node identifier for this localized resource.
    pub fn node_id(&self) -> &String {
        &self.node_id
    }

    /// Returns the resource for this localized resource.
    pub fn resource(&self) -> &Resource {
        &self.resource
    }
}

/// Trait for services that have a node identifier in distributed systems.
///
/// This trait is implemented by services that participate in distributed
/// traceability operations and need to identify themselves to remote peers.
/// The node ID is typically used in provenance records and logging to
/// track which middleware instance processed specific operations.
pub trait NodeId {
    /// Returns the unique identifier for this node in the distributed system.
    ///
    /// Node IDs should be unique across the distributed deployment and
    /// persistent across service restarts to maintain consistent provenance records.
    fn node_id(&self) -> String;
}
