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
    pub pid: i32,
    pub starttime: u64,
    pub exe_path: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Resource {
    Fd(Fd),
    Process(Process),
}

impl Resource {
    pub fn new_file(path: String) -> Self {
        Self::Fd(Fd::File(File { path }))
    }

    pub fn new_stream(local_socket: String, peer_socket: String) -> Self {
        Self::Fd(Fd::Stream(Stream {
            local_socket,
            peer_socket,
        }))
    }

    pub fn new_process(pid: i32, starttime: u64, exe_path: String) -> Self {
        Self::Process(Process {
            pid,
            starttime,
            exe_path,
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Identifier {
    pub node: String,
    pub resource: Resource,
}
