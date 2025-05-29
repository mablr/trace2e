use std::fmt::Debug;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct File {
    pub path: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Stream {
    pub local_socket: String,
    pub peer_socket: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Fd {
    File(File),
    Stream(Stream),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Process {
    pub pid: u32,
    pub starttime: u64,
    pub exe_path: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Resource {
    Fd(Fd),
    Process(Process),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Identifier {
    pub node: String,
    pub resource: Resource,
}
