//! Instruction interpreter for stde2e scenarios.
//!
//! This module provides a simple DSL for executing traced I/O operations
//! using the stde2e library. Instructions follow the format:
//!
//! `ACTION argument`
//!
//! where:
//! - ACTION := "OPEN" | "CREATE" | "BIND" | "CONNECT" | "READ" | "WRITE" | "HELP"
//! - argument := file path (for OPEN/CREATE) | socket address (for BIND/CONNECT) | resource (for READ/WRITE)
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
use stde2e::io::{Read, Write};
use trace2e_core::traceability::infrastructure::naming::{Fd, Resource};

/// Handles shared I/O buffer for read/write operations across resources
#[derive(Debug)]
pub struct BufferHandler {
    buffer: Vec<u8>,
}

impl BufferHandler {
    /// Create a new buffer handler with 4KB capacity
    pub fn new() -> Self {
        Self { buffer: Vec::with_capacity(4096) }
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
            Command::Write => 2,
        }
    }
}

/// Represents a complete instruction: ACTION RESOURCE
#[derive(Debug, Clone)]
pub struct Instruction {
    pub command: Command,
    pub target: Target,
}

#[derive(Debug, Clone, Default)]
pub enum Target {
    File(String),
    Stream(String, String),
    Socket(SocketAddr),
    #[default]
    None,
}

fn socket_target(s: &str) -> Result<Target, anyhow::Error> {
    if let Some(target) = s.strip_prefix("socket://") {
        Ok(target.parse::<SocketAddr>().map(Target::Socket)?)
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

    /// Parse an instruction string in the format "ACTION resource"
    ///
    /// # Examples
    /// - `READ -- stream://127.0.0.1:8080::192.168.1.1:9000`
    /// - `WRITE -- file:///tmp/output.txt`
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        let s = s.trim();

        // Skip empty lines and comments
        if s.is_empty() || s.starts_with('#') {
            return Ok(Instruction { command: Command::Nil, target: Default::default() });
        }

        // Split into action and resource
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
            // Parse resource, or socket
            match Resource::try_from(arg) {
                Ok(r) => r.try_into()?,
                Err(_) => socket_target(arg)
                    .map_err(|_| anyhow::anyhow!("Invalid target for READ/WRITE."))?,
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
    files: HashMap<Resource, std::fs::File>,
    streams: HashMap<Resource, std::net::TcpStream>,
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
    fn open(&mut self, arg: &str) -> anyhow::Result<()> {
        let resource = Resource::try_from(arg)?;
        let path =
            resource.path().ok_or_else(|| anyhow::anyhow!("OPEN requires a file resource"))?;

        if let Entry::Vacant(e) = self.files.entry(resource.to_owned()) {
            let f = stde2e::fs::OpenOptions::new()
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
    fn create(&mut self, arg: &str) -> anyhow::Result<()> {
        let resource = Resource::try_from(arg)?;
        let path =
            resource.path().ok_or_else(|| anyhow::anyhow!("CREATE requires a file resource"))?;

        if let Entry::Vacant(e) = self.files.entry(resource.to_owned()) {
            let f = stde2e::fs::OpenOptions::new()
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
    fn bind(&mut self, local_addr: &str) -> anyhow::Result<()> {
        let listener = stde2e::net::TcpListener::bind(local_addr)
            .map_err(|e| anyhow::anyhow!("Failed to bind to '{}': {}", local_addr, e))?;
        println!("✓ Bound to socket: {}", local_addr);
        println!("  Waiting for incoming connection...");

        // Accept connection (blocking)
        let (stream, peer_addr) = listener.accept().map_err(|e| {
            anyhow::anyhow!("Failed to accept connection on '{}': {}", local_addr, e)
        })?;
        let peer_socket = peer_addr.to_string();

        // Store stream with inferred resource
        let resource =
            Resource::try_from(format!("stream://{}::{}", local_addr, peer_socket).as_str())?;
        self.streams.insert(resource.to_owned(), stream);

        println!("✓ Stream established: {}", resource);
        Ok(())
    }

    /// Connect to a remote socket
    fn connect(&mut self, peer_socket: &str) -> anyhow::Result<()> {
        let stream = stde2e::net::TcpStream::connect(peer_socket)
            .map_err(|e| anyhow::anyhow!("Failed to connect to '{}': {}", peer_socket, e))?;
        let local_socket = stream
            .local_addr()
            .map_err(|e| anyhow::anyhow!("Failed to get local socket address: {}", e))?
            .to_string();

        // Store stream with inferred resource
        let resource =
            Resource::try_from(format!("stream://{}::{}", local_socket, peer_socket).as_str())?;
        self.streams.insert(resource.to_owned(), stream);

        println!("✓ Stream established: {}", resource);
        Ok(())
    }

    /// Execute a READ command, appending data to the shared buffer
    pub fn read(&mut self, resource: &Resource, buffer: &mut Vec<u8>) -> anyhow::Result<()> {
        let mut temp_buf = [0u8; 4096];

        match resource {
            Resource::Fd(Fd::File(file)) => {
                let f = self
                    .files
                    .get_mut(resource)
                    .ok_or_else(|| anyhow::anyhow!("File not opened: {}", file.path))?;
                let n = f.read(&mut temp_buf).map_err(|e| {
                    anyhow::anyhow!("Failed to read from file '{}': {}", file.path, e)
                })?;
                buffer.extend_from_slice(&temp_buf[..n]);
                println!("✓ Read {} bytes from file: {}", n, file.path);
                Ok(())
            }
            Resource::Fd(Fd::Stream(stream)) => {
                let s = self.streams.get_mut(resource).ok_or_else(|| {
                    anyhow::anyhow!("Stream not connected: {}", stream.peer_socket)
                })?;
                let n = s.read(&mut temp_buf).map_err(|e| {
                    anyhow::anyhow!("Failed to read from stream '{}': {}", stream.peer_socket, e)
                })?;
                buffer.extend_from_slice(&temp_buf[..n]);
                println!("✓ Read {} bytes from stream: {}", n, stream.peer_socket);
                Ok(())
            }
            _ => Err(anyhow::anyhow!("Unsupported resource type for READ operation")),
        }
    }

    /// Execute a WRITE command using the shared buffer
    pub fn write(&mut self, resource: &Resource, buffer: &[u8]) -> anyhow::Result<()> {
        match resource {
            Resource::Fd(Fd::File(file)) => {
                let f = self
                    .files
                    .get_mut(resource)
                    .ok_or_else(|| anyhow::anyhow!("File not opened: {}", file.path))?;
                let n = f.write(buffer).map_err(|e| {
                    anyhow::anyhow!("Failed to write to file '{}': {}", file.path, e)
                })?;
                println!("✓ Wrote {} bytes to file: {}", n, file.path);
                Ok(())
            }
            Resource::Fd(Fd::Stream(stream)) => {
                let s = self.streams.get_mut(resource).ok_or_else(|| {
                    anyhow::anyhow!("Stream not connected: {}", stream.peer_socket)
                })?;
                let n = s.write(buffer).map_err(|e| {
                    anyhow::anyhow!("Failed to write to stream '{}': {}", stream.peer_socket, e)
                })?;
                println!("✓ Wrote {} bytes to stream: {}", n, stream.peer_socket);
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
            (Command::Read, Target::File(path)) => {
                let resource = Resource::try_from(format!("file://{path}").as_str())?;
                self.read(&resource, buffer)
            }
            (Command::Read, Target::Stream(local, peer)) => {
                let resource = Resource::try_from(format!("stream://{local}::{peer}").as_str())?;
                self.read(&resource, buffer)
            }
            (Command::Read, _) => Err(anyhow::anyhow!("READ command requires a valid resource")),
            (Command::Write, Target::File(path)) => {
                let resource = Resource::try_from(format!("file://{path}").as_str())?;
                self.write(&resource, buffer)
            }
            (Command::Write, Target::Stream(local, peer)) => {
                let resource = Resource::try_from(format!("stream://{local}::{peer}").as_str())?;
                self.write(&resource, buffer)
            }
            (Command::Write, _) => Err(anyhow::anyhow!("WRITE command requires a valid resource")),
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
