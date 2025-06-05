use std::fmt::Debug;

use procfs::process;

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
        match process::Process::new(pid) {
            Ok(procfs_process) => {
                let starttime = procfs_process
                    .stat()
                    .map_or_else(|_| Default::default(), |stat| stat.starttime);
                let exe_path = procfs_process.exe().map_or_else(
                    |_| Default::default(),
                    |exe| exe.to_str().unwrap_or_default().to_string(),
                );
                Self::Process(Process {
                    pid,
                    starttime,
                    exe_path,
                })
            }
            Err(_) => Self::Process(Process {
                pid,
                starttime: 0,
                exe_path: String::default(),
            }),
        }
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
