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
pub enum ResourceVariant {
    Fd(Fd),
    Process(Process),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Identifier {
    pub node: String,
    pub variant: ResourceVariant,
}


#[derive(Debug, Clone, Eq, PartialEq)]
pub enum FlowType {
    Read,
    Write,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum P2mRequest {
    LocalEnroll {
        pid: u32,
        fd: i32,
        path: String,
    },
    RemoteEnroll {
        pid: u32,
        fd: i32,
        local_socket: String,
        peer_socket: String,
    },
    IoRequest {
        pid: u32,
        fd: i32,
        flow_type: FlowType,
    },
    IoReport {
        pid: u32,
        fd: i32,
        flow_id: u64,
        io_result: bool,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum P2mResponse {
    Enrolled,
    FlowGranted { flow_id: u64 },
    FlowReported,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ResourceRequest {
    GetProv,
    UpdateProv(Vec<Identifier>),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ResourceResponse {
    Prov(Vec<Identifier>),
    ProvUpdated,
}
