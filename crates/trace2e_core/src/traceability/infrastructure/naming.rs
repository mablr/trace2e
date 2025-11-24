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

use std::{
    collections::HashSet,
    fmt::{Debug, Display},
    net::SocketAddr,
};

use sysinfo::{Pid, System};

use crate::traceability::error::TraceabilityError;

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

    /// Returns the path for File variant, or None otherwise
    pub fn path(&self) -> Option<&str> {
        if let Resource::Fd(Fd::File(f)) = self { Some(&f.path) } else { None }
    }

    /// Checks if this resource represents a network stream.
    ///
    /// Returns true if the resource is a file descriptor pointing to a stream,
    /// false for files, processes, or null resources.
    pub fn is_stream(&self) -> bool {
        matches!(self, Resource::Fd(Fd::Stream(_)))
    }

    /// Returns the local socket for Stream variant, or None otherwise
    pub fn local_socket(&self) -> Option<&str> {
        if let Resource::Fd(Fd::Stream(s)) = self { Some(&s.local_socket) } else { None }
    }

    /// Returns the local socket for Stream variant, or None otherwise
    pub fn peer_socket(&self) -> Option<&str> {
        if let Resource::Fd(Fd::Stream(s)) = self { Some(&s.peer_socket) } else { None }
    }

    /// Converts this resource into a localized resource given the specified localization.
    ///
    /// It attempts to convert the resource into a localized stream resource, which infers
    /// the peer node from the stream's peer socket.
    /// Otherwise, it creates a localized resource with the specified localization..
    pub fn into_localized(self, localization: String) -> LocalizedResource {
        if let Some(localized_stream) = self.try_into_localized_peer_stream() {
            localized_stream
        } else {
            LocalizedResource::new(localization, self)
        }
    }

    /// Returns the peer stream resource if this is a stream resource.
    ///
    /// For stream resources, returns a new resource with the local and peer
    /// socket addresses swapped. This is useful for tracking bidirectional
    /// flows. Returns None for non-stream resources.
    pub fn try_into_localized_peer_stream(&self) -> Option<LocalizedResource> {
        let Resource::Fd(Fd::Stream(stream)) = self else {
            return None;
        };

        let peer_socket = stream.peer_socket.parse::<SocketAddr>().ok()?;
        let ip = peer_socket.ip();
        let node_id = if ip.is_ipv6() { format!("[{}]", ip) } else { ip.to_string() };

        Some(LocalizedResource::new(
            node_id,
            Resource::new_stream(stream.peer_socket.clone(), stream.local_socket.clone()),
        ))
    }

    /// Checks if this resource represents a system process.
    ///
    /// Returns true if the resource is a process, false for file descriptors
    /// or null resources.
    pub fn is_process(&self) -> bool {
        matches!(self, Resource::Process(_))
    }
}

impl Display for Resource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Resource::Fd(Fd::File(file)) => write!(f, "file://{}", file.path),
            Resource::Fd(Fd::Stream(stream)) => {
                write!(f, "stream://{}::::{}", stream.local_socket, stream.peer_socket)
            }
            Resource::Process(process) => {
                write!(f, "process://{}::{}::{}", process.pid, process.starttime, process.exe_path)
            }
            Resource::None => write!(f, "None"),
        }
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

impl TryFrom<&str> for Resource {
    type Error = TraceabilityError;

    /// Parse a resource string (without node_id) into a Resource.
    ///
    /// Supports the following formats:
    /// - `file:///path` - File resource
    /// - `stream://local::::peer` - Stream resource
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        if let Some(path) = s.strip_prefix("file://") {
            Ok(Resource::new_file(path.to_string()))
        } else if let Some(stream_part) = s.strip_prefix("stream://") {
            let sockets: Vec<&str> = stream_part.split("::::").collect();
            if sockets.len() != 2 {
                return Err(TraceabilityError::InvalidResourceFormat(
                    "Invalid stream format: expected 'stream://local::::peer'".to_string(),
                ));
            }
            Ok(Resource::new_stream(sockets[0].to_string(), sockets[1].to_string()))
        } else {
            Err(TraceabilityError::InvalidResourceFormat(
                "Resource must start with 'file://' or 'stream://'".to_string(),
            ))
        }
    }
}

impl TryFrom<String> for Resource {
    type Error = TraceabilityError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Resource::try_from(s.as_str())
    }
}

impl TryFrom<&str> for LocalizedResource {
    type Error = TraceabilityError;

    /// Parse a localized resource string into a LocalizedResource.
    ///
    /// Supports the format: `resource@node_id`
    /// where resource is either:
    /// - `file:///path` - File resource
    /// - `stream://local::::peer` - Stream resource
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        // Split by '@' to separate resource and node_id
        let parts: Vec<&str> = s.rsplitn(2, '@').collect();
        if parts.len() != 2 {
            return Err(TraceabilityError::InvalidResourceFormat(
                "Missing '@node_id' separator".to_string(),
            ));
        }

        let (node_id, resource_part) = (parts[0], parts[1]);
        let resource = Resource::try_from(resource_part)?;

        Ok(LocalizedResource::new(node_id.to_string(), resource))
    }
}

impl TryFrom<String> for LocalizedResource {
    type Error = TraceabilityError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        LocalizedResource::try_from(s.as_str())
    }
}

impl Display for LocalizedResource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}", self.resource, self.node_id)
    }
}

/// Wrapper type for Display implementation of Option<Resource>
///
/// Uses references to avoid cloning data when displaying.
#[derive(Debug)]
pub enum DisplayableResource<'a, T> {
    Option(&'a Option<T>),
    HashSet(&'a HashSet<T>),
    Slice(&'a [T]),
}

impl<'a> Display for DisplayableResource<'a, Resource> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DisplayableResource::Option(Some(resource)) => write!(f, "{}", resource),
            DisplayableResource::Option(None) => write!(f, "None"),
            DisplayableResource::HashSet(resources) => write!(
                f,
                "[{}]",
                resources.iter().map(|r| r.to_string()).collect::<Vec<String>>().join(", ")
            ),
            DisplayableResource::Slice(resources) => write!(
                f,
                "[{}]",
                resources.iter().map(|r| r.to_string()).collect::<Vec<String>>().join(", ")
            ),
        }
    }
}

impl<'a> Display for DisplayableResource<'a, LocalizedResource> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DisplayableResource::Option(Some(resource)) => write!(f, "{}", resource),
            DisplayableResource::Option(None) => write!(f, "None"),
            DisplayableResource::HashSet(resources) => write!(
                f,
                "[{}]",
                resources.iter().map(|r| r.to_string()).collect::<Vec<String>>().join(", ")
            ),
            DisplayableResource::Slice(resources) => write!(
                f,
                "[{}]",
                resources.iter().map(|r| r.to_string()).collect::<Vec<String>>().join(", ")
            ),
        }
    }
}

impl<'a> From<&'a Option<Resource>> for DisplayableResource<'a, Resource> {
    fn from(t: &'a Option<Resource>) -> Self {
        DisplayableResource::Option(t)
    }
}

impl<'a> From<&'a HashSet<Resource>> for DisplayableResource<'a, Resource> {
    fn from(t: &'a HashSet<Resource>) -> DisplayableResource<'a, Resource> {
        DisplayableResource::HashSet(t)
    }
}

impl<'a> From<&'a [Resource]> for DisplayableResource<'a, Resource> {
    fn from(t: &'a [Resource]) -> DisplayableResource<'a, Resource> {
        DisplayableResource::Slice(t)
    }
}

impl<'a> From<&'a Option<LocalizedResource>> for DisplayableResource<'a, LocalizedResource> {
    fn from(t: &'a Option<LocalizedResource>) -> Self {
        DisplayableResource::Option(t)
    }
}

impl<'a> From<&'a HashSet<LocalizedResource>> for DisplayableResource<'a, LocalizedResource> {
    fn from(t: &'a HashSet<LocalizedResource>) -> Self {
        DisplayableResource::HashSet(t)
    }
}

impl<'a> From<&'a [LocalizedResource]> for DisplayableResource<'a, LocalizedResource> {
    fn from(t: &'a [LocalizedResource]) -> Self {
        DisplayableResource::Slice(t)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_display() {
        let file = LocalizedResource::new(
            "127.0.0.1".to_string(),
            Resource::new_file("/tmp/test.txt".to_string()),
        );
        let stream = LocalizedResource::new(
            "127.0.0.1".to_string(),
            Resource::new_stream("127.0.0.1:8080".to_string(), "127.0.0.1:8081".to_string()),
        );
        let process_mock =
            LocalizedResource::new("127.0.0.1".to_string(), Resource::new_process_mock(1234));
        let none = LocalizedResource::new(Default::default(), Resource::None);

        assert_eq!(file.to_string(), "file:///tmp/test.txt@127.0.0.1");
        assert_eq!(stream.to_string(), "stream://127.0.0.1:8080::::127.0.0.1:8081@127.0.0.1");
        assert_eq!(process_mock.to_string(), "process://1234::0::@127.0.0.1");
        assert_eq!(none.to_string(), "None@");
    }

    #[test]
    fn test_displayable_resource() {
        let file = LocalizedResource::new(
            "127.0.0.1".to_string(),
            Resource::new_file("/tmp/test.txt".to_string()),
        );
        let stream = LocalizedResource::new(
            "127.0.0.1".to_string(),
            Resource::new_stream("127.0.0.1:8080".to_string(), "127.0.0.1:8081".to_string()),
        );
        let process_mock =
            LocalizedResource::new("127.0.0.1".to_string(), Resource::new_process_mock(1234));
        let none = LocalizedResource::new(Default::default(), Resource::None);

        // localized resources - test direct resource display
        assert_eq!(file.to_string(), "file:///tmp/test.txt@127.0.0.1");
        assert_eq!(process_mock.to_string(), "process://1234::0::@127.0.0.1");
        assert_eq!(stream.to_string(), "stream://127.0.0.1:8080::::127.0.0.1:8081@127.0.0.1");
        assert_eq!(none.to_string(), "None@");

        // test DisplayableResource with vector of cloned resources for display
        // In production code, we pass references to avoid clones (as shown in other files)
        assert_eq!(
            DisplayableResource::from(vec![file, process_mock, stream, none].as_slice())
                .to_string(),
            "[file:///tmp/test.txt@127.0.0.1, process://1234::0::@127.0.0.1, stream://127.0.0.1:8080::::127.0.0.1:8081@127.0.0.1, None@]"
        );
    }

    #[test]
    fn test_resource_try_from_file() {
        let resource = Resource::try_from("file:///tmp/test.txt").unwrap();
        assert!(resource.is_file());
    }

    #[test]
    fn test_resource_try_from_stream() {
        let resource = Resource::try_from("stream://127.0.0.1:8080::::192.168.1.1:9000").unwrap();
        assert!(resource.is_stream());
    }

    #[test]
    fn test_resource_try_from_string() {
        let resource = Resource::try_from("file:///tmp/test.txt".to_string()).unwrap();
        assert!(resource.is_file());
    }

    #[test]
    fn test_localized_resource_try_from_file() {
        let localized = LocalizedResource::try_from("file:///tmp/test.txt@127.0.0.1").unwrap();
        assert_eq!(localized.node_id(), "127.0.0.1");
        assert!(localized.resource().is_file());
    }

    #[test]
    fn test_localized_resource_try_from_stream() {
        let localized =
            LocalizedResource::try_from("stream://127.0.0.1:8080::::192.168.1.1:9000@10.0.0.1")
                .unwrap();
        assert_eq!(localized.node_id(), "10.0.0.1");
        assert!(localized.resource().is_stream());
    }

    #[test]
    fn test_localized_resource_try_from_string() {
        let localized =
            LocalizedResource::try_from("file:///tmp/test.txt@127.0.0.1".to_string()).unwrap();
        assert_eq!(localized.node_id(), "127.0.0.1");
        assert!(localized.resource().is_file());
    }

    #[test]
    fn test_resource_try_from_invalid() {
        let result = Resource::try_from("invalid_resource");
        assert!(result.is_err());
        assert!(matches!(result, Err(TraceabilityError::InvalidResourceFormat(_))));

        let result = Resource::try_from("stream://no_peer");
        assert!(result.is_err());
        assert!(matches!(result, Err(TraceabilityError::InvalidResourceFormat(_))));
    }

    #[test]
    fn test_localized_resource_try_from_invalid() {
        let result = LocalizedResource::try_from("file:///tmp/test.txt");
        assert!(result.is_err());
        assert!(matches!(result, Err(TraceabilityError::InvalidResourceFormat(_))));

        let result = LocalizedResource::try_from("stream://127.0.0.1:8080::::192.168.1.1:9000");
        assert!(result.is_err());
        assert!(matches!(result, Err(TraceabilityError::InvalidResourceFormat(_))));
    }
}
