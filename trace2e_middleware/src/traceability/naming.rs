use std::fmt::Debug;

use sysinfo::{Pid, System};

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
    pub starttime: u64,
    pub exe_path: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Default)]
pub enum Resource {
    Fd(Fd),
    Process(Process),
    #[default]
    None,
}

impl Resource {
    pub fn new_file(path: String) -> Self {
        Self::Fd(Fd::File(File { path }))
    }

    pub fn new_stream(local_socket: String, peer_socket: String) -> Self {
        Self::Fd(Fd::Stream(Stream { local_socket, peer_socket }))
    }

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

    pub fn new_process_mock(pid: i32) -> Self {
        Self::Process(Process { pid, starttime: 0, exe_path: String::new() })
    }

    pub fn is_file(&self) -> bool {
        matches!(self, Resource::Fd(Fd::File(_)))
    }

    pub fn is_stream(&self) -> Option<Self> {
        if let Resource::Fd(Fd::Stream(stream)) = self {
            Some(Self::new_stream(stream.peer_socket.clone(), stream.local_socket.clone()))
        } else {
            None
        }
    }

    pub fn is_process(&self) -> bool {
        matches!(self, Resource::Process(_))
    }
}

pub trait NodeId {
    fn node_id(&self) -> String;
}
