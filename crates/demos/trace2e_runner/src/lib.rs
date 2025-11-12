//! Instruction interpreter for stde2e scenarios.
//!
//! This module provides a simple DSL for executing traced I/O operations
//! using the stde2e library. Instructions follow the format:
//!
//! `ACTION resource`
//!
//! where:
//! - ACTION := "READ" | "WRITE"
//! - resource := "file:///path" | "stream://local_socket::peer_socket"
//!
//! Examples:
//! - `READ stream://192.168.1.1:8080::192.168.1.2:9000`
//! - `WRITE file:///tmp/output.txt`

use anyhow::Context;
use std::collections::HashMap;
use std::convert::TryFrom;
use trace2e_core::traceability::infrastructure::naming::Resource;

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

    /// Get the buffer content as bytes
    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }

    /// Get mutable reference to buffer
    pub fn buffer_mut(&mut self) -> &mut Vec<u8> {
        &mut self.buffer
    }

    /// Clear the buffer for reuse
    pub fn clear(&mut self) {
        self.buffer.clear();
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
    Read,
    Write,
    Help,
}

impl Command {
    /// Parse a command from a string
    fn parse(s: &str) -> anyhow::Result<Self> {
        match s.to_uppercase().as_str() {
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
            Command::Read => 2,
            Command::Write => 2,
        }
    }
}

/// Represents a complete instruction: ACTION RESOURCE
#[derive(Debug, Clone)]
pub struct Instruction {
    pub command: Command,
    pub resource: Option<Resource>,
}

impl TryFrom<&str> for Instruction {
    type Error = anyhow::Error;

    /// Parse an instruction string in the format "ACTION resource"
    ///
    /// # Examples
    /// - `READ -- stream://127.0.0.1:8080::192.168.1.1:9000`
    /// - `WRITE -- file:///tmp/output.txt`
    fn try_from(s: &str) -> Result<Instruction, Self::Error> {
        let s = s.trim();

        // Skip empty lines and comments
        if s.is_empty() || s.starts_with('#') {
            return Ok(Instruction { command: Command::Nil, resource: Some(Resource::default()) });
        }

        // Split into action and resource
        let parts: Vec<&str> = s.splitn(2, char::is_whitespace).collect();
        if parts.is_empty() {
            return Err(anyhow::anyhow!("Invalid instruction format"));
        }

        // Parse command
        let command = Command::parse(parts[0])?;
        let mut resource: Option<Resource> = None;

        // Validate arity
        if command.arity() != parts.len() {
            return Err(anyhow::anyhow!(
                "Invalid number of arguments for command: {}, expected {}, got {}",
                parts[0],
                command.arity(),
                parts.len()
            ));
        } else if command.arity() == 2 {
            // Parse resource
            resource = Resource::try_from(parts[1].trim())
                .map_err(|e| {
                    anyhow::anyhow!("Failed to parse resource for command {}: {}", parts[0], e)
                })
                .ok();
        }

        Ok(Instruction { command, resource })
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

    /// Get immutable reference to the buffer
    pub fn buffer(&self) -> &[u8] {
        self.buffer.buffer()
    }

    /// Get mutable reference to the buffer
    pub fn buffer_mut(&mut self) -> &mut Vec<u8> {
        self.buffer.buffer_mut()
    }

    /// Clear the shared buffer for reuse
    pub fn clear_buffer(&mut self) {
        self.buffer.clear();
    }

    /// Execute a READ command, using the shared buffer
    pub fn read(&mut self, resource: &Resource) -> anyhow::Result<()> {
        self.resources.read(resource, self.buffer.buffer_mut())
    }

    /// Execute a WRITE command, using the shared buffer
    pub fn write(&mut self, resource: &Resource) -> anyhow::Result<()> {
        self.resources.write(resource, self.buffer.buffer_mut())
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
    println!(" $ READ <resource>    # Read from the specified resource");
    println!(" $ WRITE <resource>   # Write to the specified resource");
    println!(" $ HELP               # Show this help message");
    println!(" $ # [comment]        # Comment line");
    println!(" $                    # No operation");
    println!("");
    println!("Resource format:");
    println!(" - file:///path/to/file");
    println!(" - stream://local_socket::peer_socket");
    println!("");
}

impl Resources {
    /// Get or open a file handle, creating it if necessary
    fn get_or_open_file(
        &mut self,
        resource: &Resource,
        path: &str,
    ) -> anyhow::Result<&mut std::fs::File> {
        if !self.files.contains_key(resource) {
            let f = stde2e::fs::File::open(path)
                .with_context(|| format!("Failed to open file: {}", path))?;
            println!("✓ Opened file: {}", path);
            self.files.insert(resource.to_owned(), f);
        }
        Ok(self.files.get_mut(resource).unwrap())
    }

    /// Get or open a stream handle, creating it if necessary
    fn get_or_open_stream(
        &mut self,
        resource: &Resource,
        peer_socket: &str,
    ) -> anyhow::Result<&mut std::net::TcpStream> {
        if !self.streams.contains_key(resource) {
            let s = stde2e::net::TcpStream::connect(peer_socket)
                .with_context(|| format!("Failed to connect to: {}", peer_socket))?;
            println!("✓ Connected to stream: {}", peer_socket);
            self.streams.insert(resource.to_owned(), s);
        }
        Ok(self.streams.get_mut(resource).unwrap())
    }

    /// Execute a READ command, appending data to the shared buffer
    pub fn read(&mut self, resource: &Resource, buffer: &mut Vec<u8>) -> anyhow::Result<()> {
        use stde2e::io::Read;
        use trace2e_core::traceability::infrastructure::naming::Fd;

        let mut temp_buf = [0u8; 4096];

        match resource {
            Resource::Fd(Fd::File(file)) => {
                let f = self.get_or_open_file(resource, &file.path)?;
                let n = f
                    .read(&mut temp_buf)
                    .with_context(|| format!("Failed to read from file: {}", file.path))?;
                buffer.extend_from_slice(&temp_buf[..n]);
                println!("✓ Read {} bytes from file: {}", n, file.path);
                Ok(())
            }
            Resource::Fd(Fd::Stream(stream)) => {
                let s = self.get_or_open_stream(resource, &stream.peer_socket)?;
                let n = s.read(&mut temp_buf).with_context(|| {
                    format!("Failed to read from stream: {}", stream.peer_socket)
                })?;
                buffer.extend_from_slice(&temp_buf[..n]);
                println!("✓ Read {} bytes from stream: {}", n, stream.peer_socket);
                Ok(())
            }
            _ => Err(anyhow::anyhow!("Unsupported resource type for READ operation")),
        }
    }

    /// Execute a WRITE command using the shared buffer
    #[allow(clippy::ptr_arg)]
    pub fn write(&mut self, resource: &Resource, buffer: &Vec<u8>) -> anyhow::Result<()> {
        use stde2e::io::Write;
        use trace2e_core::traceability::infrastructure::naming::Fd;

        match resource {
            Resource::Fd(Fd::File(file)) => {
                let f = self.get_or_open_file(resource, &file.path)?;
                let n = f
                    .write(buffer)
                    .with_context(|| format!("Failed to write to file: {}", file.path))?;
                println!("✓ Wrote {} bytes to file: {}", n, file.path);
                Ok(())
            }
            Resource::Fd(Fd::Stream(stream)) => {
                let s = self.get_or_open_stream(resource, &stream.peer_socket)?;
                let n = s.write(buffer).with_context(|| {
                    format!("Failed to write to stream: {}", stream.peer_socket)
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
        match instruction.command {
            Command::Nil => Ok(()),
            Command::Read => {
                if let Some(resource) = &instruction.resource {
                    self.read(resource, buffer)
                } else {
                    Err(anyhow::anyhow!("READ command requires a valid resource"))
                }
            }
            Command::Write => {
                if let Some(resource) = &instruction.resource {
                    self.write(resource, buffer)
                } else {
                    Err(anyhow::anyhow!("WRITE command requires a valid resource"))
                }
            }
            Command::Help => {
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
        assert!(instr.resource.is_some_and(|r| r.is_stream()));
    }

    #[test]
    fn test_parse_instruction_write_file() {
        let instr = Instruction::try_from("WRITE file:///tmp/output.txt@localhost").unwrap();
        assert_eq!(instr.command, Command::Write);
        assert!(instr.resource.is_some_and(|r| r.is_file()));
    }

    #[test]
    fn test_parse_instruction_invalid_command() {
        let result = Instruction::try_from("INVALID file:///tmp/test.txt@127.0.0.1");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_instruction_missing_resource() {
        let result = Instruction::try_from("OPEN");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_instruction_invalid_resource() {
        let result = Instruction::try_from("OPEN invalid_resource");
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
