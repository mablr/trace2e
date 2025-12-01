//! Instruction interpreter for stde2e scenarios.
//!
//! This module provides a simple DSL for executing traced I/O operations
//! using the stde2e library. Instructions follow the format:
//!
//! `ACTION argument`
//!
//! where:
//! - ACTION := "OPEN" | "CREATE" | "BIND" | "CONNECT" | "READ" | "WRITE" | "HELP"
//! - argument := file resource (for OPEN/CREATE) | socket address (for BIND/CONNECT/READ/WRITE) | any resource (for READ/WRITE)
//!
//! Examples:
//! - `OPEN /tmp/input.txt`
//! - `CREATE /tmp/output.txt`
//! - `BIND 127.0.0.1:8080`
//! - `CONNECT 192.168.1.100:9000`
//! - `READ file:///tmp/input.txt`
//! - `WRITE stream://127.0.0.1:12345::192.168.1.100:9000`

use std::collections::hash_map::Entry;
use std::convert::TryFrom;
use std::{collections::HashMap, net::SocketAddr};
use stde2e::{
    fs::OpenOptions,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
};
use trace2e_core::traceability::infrastructure::naming::Resource;

const DEFAULT_SLEEP_TIME: u64 = 1000; // Default sleep time in milliseconds

/// Handles shared I/O buffer for read/write operations across resources
#[derive(Debug)]
pub struct BufferHandler {
    buffer: Vec<u8>,
}

impl BufferHandler {
    /// Create a new buffer handler with 4KB capacity
    pub fn new() -> Self {
        let mut buffer = Vec::with_capacity(4096);
        buffer.extend("abc".as_bytes()); // Pre-fill with some dummy data
        Self { buffer }
    }

    /// Get mutable reference to buffer
    pub fn buffer_mut(&mut self) -> &mut Vec<u8> {
        &mut self.buffer
    }
}

impl Default for BufferHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents a command action
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Nil,
    Open,
    Create,
    Bind,
    Connect,
    Read,
    Write,
    Sleep,
    Help,
}

impl Command {
    /// Parse a command from a string
    fn parse(s: &str) -> anyhow::Result<Self> {
        match s.to_uppercase().as_str() {
            "OPEN" | "O" => Ok(Command::Open),
            "CREATE" | "C" => Ok(Command::Create),
            "BIND" | "B" => Ok(Command::Bind),
            "CONNECT" | "CN" => Ok(Command::Connect),
            "READ" | "R" => Ok(Command::Read),
            "WRITE" | "W" => Ok(Command::Write),
            "SLEEP" | "S" => Ok(Command::Sleep),
            "HELP" | "H" | "?" => Ok(Command::Help),
            _ => Err(anyhow::anyhow!("Unknown command: {}", s)),
        }
    }

    fn arity(&self) -> usize {
        match self {
            Command::Nil => 1,
            Command::Help => 1,
            Command::Open => 2,
            Command::Create => 2,
            Command::Bind => 2,
            Command::Connect => 2,
            Command::Read => 2,
            Command::Sleep => 2,
            Command::Write => 2,
        }
    }
}

/// Represents a complete instruction: CMD TARGET
#[derive(Debug, Clone)]
pub struct Instruction {
    pub command: Command,
    pub target: Target,
}

#[derive(Debug, Clone, Default)]
pub enum Target {
    File(String),
    Stream(String, String),
    Socket(String),
    Time(u64),
    #[default]
    None,
}

fn socket_target(s: &str) -> Result<Target, anyhow::Error> {
    if let Some(target) = s.strip_prefix("socket://") {
        Ok(target.parse::<SocketAddr>().map(|s| Target::Socket(s.to_string()))?)
    } else {
        Err(anyhow::anyhow!("Invalid socket: {s}"))
    }
}

impl TryFrom<Resource> for Target {
    type Error = anyhow::Error;
    fn try_from(value: Resource) -> Result<Self, Self::Error> {
        if let Some(path) = value.path() {
            Ok(Self::File(path.to_string()))
        } else if let (Some(local), Some(peer)) = (value.local_socket(), value.peer_socket()) {
            Ok(Self::Stream(local.to_string(), peer.to_string()))
        } else {
            Err(anyhow::anyhow!("The provided Resource is not a valid target."))
        }
    }
}

impl TryFrom<&str> for Instruction {
    type Error = anyhow::Error;

    /// Parse an instruction string in the format "CMD TARGET"
    ///
    /// # Examples
    /// - `READ stream://127.0.0.1:8080::192.168.1.1:9000`
    /// - `WRITE file:///tmp/output.txt`
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        let s = s.trim();

        // Skip empty lines and comments
        if s.is_empty() || s.starts_with('#') {
            return Ok(Instruction { command: Command::Nil, target: Default::default() });
        }

        // Split into command and target
        let parts: Vec<&str> = s.splitn(2, char::is_whitespace).collect();
        if parts.is_empty() {
            return Err(anyhow::anyhow!("Invalid instruction format"));
        }

        // Parse command
        let command = Command::parse(parts[0])?;
        let target = if command.arity() != parts.len() {
            return Err(anyhow::anyhow!(
                "Invalid number of arguments for command: {}, expected {}, got {}",
                parts[0],
                command.arity(),
                parts.len()
            ));
        } else if command.arity() == 2 {
            let arg = parts[1].trim();
            if command == Command::Sleep {
                // Sleep command does not require a target
                let duration = arg.parse::<u64>().unwrap_or(DEFAULT_SLEEP_TIME);
                return Ok(Instruction { command, target: Target::Time(duration) });
            } else {
                // Parse resource, or socket
                match Resource::try_from(arg) {
                    Ok(r) => r.try_into()?,
                    Err(_) => socket_target(arg)
                        .map_err(|_| anyhow::anyhow!("Invalid target for READ/WRITE."))?,
                }
            }
        } else {
            unreachable!()
        };

        Ok(Instruction { command, target })
    }
}

impl TryFrom<String> for Instruction {
    type Error = anyhow::Error;

    fn try_from(s: String) -> Result<Instruction, anyhow::Error> {
        Instruction::try_from(s.as_str())
    }
}

/// Tracks opened file handles and network streams
#[derive(Debug, Default)]
pub struct Resources {
    files: HashMap<String, std::fs::File>,
    streams: HashMap<Resource, std::net::TcpStream>,
    local_peers: HashMap<String, Resource>,
    remote_peers: HashMap<String, Resource>,
}

/// Combines resource management with shared I/O buffer handling
#[derive(Debug)]
pub struct IoHandler {
    resources: Resources,
    buffer: BufferHandler,
}

impl IoHandler {
    /// Create a new IoHandler with default resources and buffer
    pub fn new() -> Self {
        Self { resources: Resources::default(), buffer: BufferHandler::new() }
    }

    /// Execute an instruction using the managed resources and buffer
    pub fn execute(&mut self, instruction: &Instruction) -> anyhow::Result<()> {
        self.resources.execute(instruction, self.buffer.buffer_mut())
    }
}

impl Default for IoHandler {
    fn default() -> Self {
        Self::new()
    }
}

fn print_help() {
    println!("Available instructions:");
    println!("Resource operations:");
    println!(" $ OPEN <path>                    # Open existing file for reading");
    println!(" $ CREATE <path>                  # Create new file for writing");
    println!(" $ BIND <socket>                  # Bind to socket and wait for connection");
    println!(" $ CONNECT <socket>               # Connect to remote socket");
    println!();
    println!("I/O operations:");
    println!(" $ READ <resource>                # Read from the specified resource");
    println!(" $ WRITE <resource>               # Write to the specified resource");
    println!();
    println!("Utility:");
    println!(" $ HELP                           # Show this help message");
    println!(" $ # [comment]                    # Comment line");
    println!(" $                                # No operation");
    println!();
    println!("Argument format:");
    println!(" - File path: /path/to/file or /path/to/file");
    println!(" - Socket address: 127.0.0.1:8080 or hostname:port");
    println!(" - File resource: file:///path/to/file");
    println!(" - Stream resource: stream://local_socket::peer_socket");
    println!();
}

impl Resources {
    /// Open an existing file for reading and writing
    fn open(&mut self, path: &str) -> anyhow::Result<()> {
        if let Entry::Vacant(e) = self.files.entry(path.to_owned()) {
            let f = OpenOptions::new()
                .read(true)
                .write(true)
                .open(path)
                .map_err(|e| anyhow::anyhow!("Failed to open file '{}': {}", path, e))?;
            println!("✓ Opened file: {}", path);
            e.insert(f);
        } else {
            println!("⚠ File already open: {}", path);
        }
        Ok(())
    }

    /// Create a new file for reading and writing, the file is truncated if it already exists.
    fn create(&mut self, path: &str) -> anyhow::Result<()> {
        if let Entry::Vacant(e) = self.files.entry(path.to_owned()) {
            let f = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open(path)
                .map_err(|e| anyhow::anyhow!("Failed to create file '{}': {}", path, e))?;
            println!("✓ Created file: {}", path);
            e.insert(f);
        } else {
            println!("⚠ File already open: {}", path);
        }
        Ok(())
    }

    /// Bind to a socket and wait for incoming connection
    fn bind(&mut self, local_socket: &str) -> anyhow::Result<()> {
        let listener = TcpListener::bind(local_socket)
            .map_err(|e| anyhow::anyhow!("Failed to bind to '{}': {}", local_socket, e))?;
        println!("✓ Bound to socket: {}", local_socket);
        println!("  Waiting for incoming connection...");

        // Accept connection (blocking)
        let (stream, peer_socket) = listener.accept().map_err(|e| {
            anyhow::anyhow!("Failed to accept connection on '{}': {}", local_socket, e)
        })?;

        // Store stream with inferred resource
        let resource = Resource::new_stream(local_socket.to_string(), peer_socket.to_string());
        self.streams.insert(resource.to_owned(), stream);
        self.local_peers.insert(local_socket.to_string(), resource.clone());
        self.remote_peers.insert(peer_socket.to_string(), resource.clone());

        println!("✓ Stream established: {}", resource);
        Ok(())
    }

    /// Connect to a remote socket
    fn connect(&mut self, peer_socket: &str) -> anyhow::Result<()> {
        let stream = TcpStream::connect(peer_socket)
            .map_err(|e| anyhow::anyhow!("Failed to connect to '{}': {}", peer_socket, e))?;
        let local_socket = stream
            .local_addr()
            .map_err(|e| anyhow::anyhow!("Failed to get local socket address: {}", e))?
            .to_string();

        // Store stream with inferred resource
        let resource = Resource::new_stream(local_socket.to_string(), peer_socket.to_string());
        self.streams.insert(resource.to_owned(), stream);
        self.local_peers.insert(local_socket.to_string(), resource.clone());
        self.remote_peers.insert(peer_socket.to_string(), resource.clone());

        println!("✓ Stream established: {}", resource);
        Ok(())
    }

    /// Execute a READ command, appending data to the shared buffer
    pub fn read(&mut self, target: &Target, buffer: &mut Vec<u8>) -> anyhow::Result<()> {
        let mut temp_buf = [0u8; 4096];

        match target {
            Target::File(file_path) => {
                let file = self
                    .files
                    .get_mut(file_path)
                    .ok_or_else(|| anyhow::anyhow!("File not opened: {}", file_path))?;
                let n = file.read(&mut temp_buf).map_err(|e| {
                    anyhow::anyhow!("Failed to read from file '{}': {}", file_path, e)
                })?;
                buffer.extend_from_slice(&temp_buf[..n]);
                println!("✓ Read {} bytes from file: {}", n, file_path);
                Ok(())
            }
            Target::Socket(local_peer) | Target::Stream(local_peer, _) => {
                let resource = self.local_peers.get_mut(local_peer).ok_or_else(|| {
                    anyhow::anyhow!("No stream established for socket: {}", local_peer)
                })?;
                let stream = self
                    .streams
                    .get_mut(resource)
                    .ok_or_else(|| anyhow::anyhow!("Stream doesn't exist: {}", resource))?;
                match stream.read(&mut temp_buf) {
                    Ok(n) => {
                        buffer.extend_from_slice(&temp_buf[..n]);
                        println!("✓ Read {} bytes from socket: {}", n, local_peer);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                        return Err(anyhow::anyhow!("Read refused on: {}", resource));
                    }
                    Err(_) => {
                        // Silently ignore other read errors
                    }
                }
                Ok(())
            }
            _ => Err(anyhow::anyhow!("Unsupported resource type for READ operation")),
        }
    }

    /// Execute a WRITE command using the shared buffer
    pub fn write(&mut self, target: &Target, buffer: &[u8]) -> anyhow::Result<()> {
        match target {
            Target::File(file_path) => {
                let file = self
                    .files
                    .get_mut(file_path)
                    .ok_or_else(|| anyhow::anyhow!("File not opened: {}", file_path))?;
                let n = file.write(buffer).map_err(|e| {
                    anyhow::anyhow!("Failed to write to file '{}': {}", file_path, e)
                })?;
                println!("✓ Wrote {} bytes to file: {}", n, file_path);
                Ok(())
            }
            Target::Socket(remote_peer) | Target::Stream(_, remote_peer) => {
                let resource = self.remote_peers.get_mut(remote_peer).ok_or_else(|| {
                    anyhow::anyhow!("No stream established for socket: {}", remote_peer)
                })?;
                let stream = self
                    .streams
                    .get_mut(resource)
                    .ok_or_else(|| anyhow::anyhow!("Stream not connected: {}", resource))?;
                match stream.write(buffer) {
                    Ok(n) => {
                        println!("✓ Wrote {} bytes to stream: {}", n, resource);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                        return Err(anyhow::anyhow!("Write refused on: {}", resource));
                    }
                    Err(_) => {
                        // Silently ignore other write errors
                    }
                }
                Ok(())
            }
            _ => Err(anyhow::anyhow!("Unsupported resource type for WRITE operation")),
        }
    }

    /// Execute an instruction with a shared buffer
    pub fn execute(
        &mut self,
        instruction: &Instruction,
        buffer: &mut Vec<u8>,
    ) -> anyhow::Result<()> {
        match (&instruction.command, &instruction.target) {
            (Command::Nil, _) => Ok(()),
            (Command::Open, Target::File(path)) => self.open(path),
            (Command::Open, _) => Err(anyhow::anyhow!("OPEN command requires a file path")),
            (Command::Create, Target::File(path)) => self.create(path),
            (Command::Create, _) => Err(anyhow::anyhow!("CREATE command requires a file path")),
            (Command::Bind, Target::Socket(addr)) => self.bind(&addr.to_string()),
            (Command::Bind, _) => Err(anyhow::anyhow!("BIND command requires a socket address")),
            (Command::Connect, Target::Socket(addr)) => self.connect(&addr.to_string()),
            (Command::Connect, _) => {
                Err(anyhow::anyhow!("CONNECT command requires a socket address"))
            }
            (Command::Read, t) => self.read(t, buffer),
            (Command::Write, t) => self.write(t, buffer),
            (Command::Sleep, Target::Time(duration)) => {
                std::thread::sleep(std::time::Duration::from_millis(*duration));
                println!("✓ Slept for {} milliseconds", duration);
                Ok(())
            }
            (Command::Sleep, _) => {
                Err(anyhow::anyhow!("SLEEP command requires a time duration in milliseconds"))
            }
            (Command::Help, _) => {
                print_help();
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_instruction_read_stream() {
        let instr =
            Instruction::try_from("READ stream://127.0.0.1:8080::192.168.1.1:9000@10.0.0.1")
                .unwrap();
        assert_eq!(instr.command, Command::Read);
        assert!(matches!(instr.target, Target::Stream(_, _)));
    }

    #[test]
    fn test_parse_instruction_write_file() {
        let instr = Instruction::try_from("WRITE file:///tmp/output.txt@localhost").unwrap();
        assert_eq!(instr.command, Command::Write);
        assert!(matches!(instr.target, Target::File(_)));
    }

    #[test]
    fn test_parse_instruction_open_file() {
        let instr = Instruction::try_from("OPEN /tmp/test.txt").unwrap();
        assert_eq!(instr.command, Command::Open);
        assert!(matches!(instr.target, Target::File(ref p) if p == "/tmp/test.txt"));
    }

    #[test]
    fn test_parse_instruction_create_file() {
        let instr = Instruction::try_from("CREATE /tmp/output.txt").unwrap();
        assert_eq!(instr.command, Command::Create);
        assert!(matches!(instr.target, Target::File(ref p) if p == "/tmp/output.txt"));
    }

    #[test]
    fn test_parse_instruction_bind_socket() {
        let instr = Instruction::try_from("BIND 127.0.0.1:8080").unwrap();
        assert_eq!(instr.command, Command::Bind);
        assert!(matches!(instr.target, Target::Socket(_)));
    }

    #[test]
    fn test_parse_instruction_connect_socket() {
        let instr = Instruction::try_from("CONNECT 192.168.1.100:9000").unwrap();
        assert_eq!(instr.command, Command::Connect);
        assert!(matches!(instr.target, Target::Socket(_)));
    }

    #[test]
    fn test_parse_instruction_invalid_command() {
        let result = Instruction::try_from("INVALID /tmp/test.txt");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_instruction_missing_resource() {
        let result = Instruction::try_from("OPEN");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_instruction_missing_socket() {
        let result = Instruction::try_from("CONNECT");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_instruction_empty_line() {
        let result = Instruction::try_from("").unwrap();
        assert!(result.command == Command::Nil);
    }

    #[test]
    fn test_parse_instruction_comment() {
        let result = Instruction::try_from("# this is a comment").unwrap();
        assert!(result.command == Command::Nil);
    }
}
