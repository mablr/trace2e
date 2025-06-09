use std::fmt::Debug;

use libproc::proc_pid::pidpath;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct File {
    pub path: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Stream {
    pub local_socket: String,
    pub peer_socket: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Fd {
    File(File),
    Stream(Stream),
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Process {
    pub pid: i32,
    pub exe_path: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Resource {
    Fd(Fd),
    Process(Process),
    None,
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

    pub fn new_process(pid: i32) -> Self {
        let starttime = 0;
        let exe_path = pidpath(pid).unwrap_or_default();
        Self::Process(Process { pid, exe_path })
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Identifier {
    pub node: String,
    pub resource: Resource,
}

impl Identifier {
    pub fn new(node: String, resource: Resource) -> Self {
        Self { node, resource }
    }

    pub fn new_none() -> Self {
        Self {
            node: String::default(),
            resource: Resource::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_process() {
        println!("{:?}", Resource::new_process(1));
    }
}
