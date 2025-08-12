use crate::traceability::naming::{Fd, Resource};

pub(super) struct FileMapping {
    pid: i32,
    fd: i32,
    process: Resource,
    file: Resource,
}

pub(super) struct StreamMapping {
    pid: i32,
    fd: i32,
    process: Resource,
    stream: Resource,
}

impl FileMapping {
    pub fn new(pid: i32, fd: i32, path: &str) -> Self {
        Self {
            pid,
            fd,
            process: Resource::new_process(pid),
            file: Resource::new_file(path.to_string()),
        }
    }
    pub fn pid(&self) -> i32 {
        self.pid
    }
    pub fn fd(&self) -> i32 {
        self.fd
    }
    pub fn file_path(&self) -> String {
        match &self.file {
            Resource::Fd(Fd::File(file)) => file.path.clone(),
            _ => panic!("FileMapping is not a file"),
        }
    }
    pub fn process(&self) -> Resource {
        self.process.clone()
    }
    pub fn file(&self) -> Resource {
        self.file.clone()
    }
}

impl StreamMapping {
    pub fn new(pid: i32, fd: i32, local_socket: &str, peer_socket: &str) -> Self {
        Self {
            pid,
            fd,
            process: Resource::new_process(pid),
            stream: Resource::new_stream(local_socket.to_string(), peer_socket.to_string()),
        }
    }
    pub fn pid(&self) -> i32 {
        self.pid
    }
    pub fn fd(&self) -> i32 {
        self.fd
    }
    pub fn stream_local_socket(&self) -> String {
        match &self.stream {
            Resource::Fd(Fd::Stream(stream)) => stream.local_socket.clone(),
            _ => panic!("StreamMapping is not a stream"),
        }
    }
    pub fn stream_peer_socket(&self) -> String {
        match &self.stream {
            Resource::Fd(Fd::Stream(stream)) => stream.peer_socket.clone(),
            _ => panic!("StreamMapping is not a stream"),
        }
    }
    pub fn process(&self) -> Resource {
        self.process.clone()
    }
    pub fn stream(&self) -> Resource {
        self.stream.clone()
    }
}

macro_rules! local_enroll {
    ($p2m:expr, $mapping:expr) => {
        assert_eq!(
            $p2m.call(P2mRequest::LocalEnroll {
                pid: $mapping.pid(),
                fd: $mapping.fd(),
                path: $mapping.file_path(),
            })
            .await
            .unwrap(),
            P2mResponse::Ack
        )
    };
}

macro_rules! remote_enroll {
    ($p2m:expr, $mapping:expr) => {
        assert_eq!(
            $p2m.call(P2mRequest::RemoteEnroll {
                pid: $mapping.pid(),
                fd: $mapping.fd(),
                local_socket: $mapping.stream_local_socket(),
                peer_socket: $mapping.stream_peer_socket(),
            })
            .await
            .unwrap(),
            P2mResponse::Ack
        )
    };
}

macro_rules! write_request {
    ($p2m:expr, $mapping:expr) => {
        match $p2m
            .call(P2mRequest::IoRequest {
                pid: $mapping.pid(),
                fd: $mapping.fd(),
                output: true,
            })
            .await
        {
            Ok(P2mResponse::Grant(flow_id)) => flow_id,
            _ => u128::MAX, // This means there was a policy violation or an error
        }
    };
}

macro_rules! read_request {
    ($p2m:expr, $mapping:expr) => {
        match $p2m
            .call(P2mRequest::IoRequest {
                pid: $mapping.pid(),
                fd: $mapping.fd(),
                output: false,
            })
            .await
        {
            Ok(P2mResponse::Grant(flow_id)) => flow_id,
            _ => u128::MAX, // This means there was a policy violation or an error
        }
    };
}

macro_rules! io_report {
    ($p2m:expr, $mapping:expr, $flow_id:expr, $result:expr) => {
        // If flow_id is u128::MAX, it means there was a policy violation or an error, do not report flow_id
        assert_ne!($flow_id, u128::MAX);
        assert_eq!(
            $p2m.call(P2mRequest::IoReport {
                pid: $mapping.pid(),
                fd: $mapping.fd(),
                grant_id: $flow_id,
                result: $result,
            })
            .await
            .unwrap(),
            P2mResponse::Ack
        )
    };
}

macro_rules! read {
    ($p2m:expr, $mapping:expr) => {
        let flow_id = read_request!($p2m, $mapping);
        io_report!($p2m, $mapping, flow_id, true);
    };
}

macro_rules! write {
    ($p2m:expr, $mapping:expr) => {
        let flow_id = write_request!($p2m, $mapping);
        io_report!($p2m, $mapping, flow_id, true);
    };
}

macro_rules! assert_provenance {
    ($o2m:expr, $resource:expr, $provenance:expr) => {
        assert_eq!(
            $o2m.call(O2mRequest::GetReferences($resource))
                .await
                .unwrap(),
            O2mResponse::References($provenance)
        )
    };
}

macro_rules! assert_policies {
    ($o2m:expr, $resource_set:expr, $policy_set:expr) => {
        assert_eq!(
            $o2m.call(O2mRequest::GetPolicies($resource_set))
                .await
                .unwrap(),
            O2mResponse::Policies($policy_set)
        )
    };
}

macro_rules! set_policy {
    ($o2m:expr, $resource:expr, $policy:expr) => {
        assert_eq!(
            $o2m.call(O2mRequest::SetPolicy {
                resource: $resource,
                policy: $policy
            })
            .await
            .unwrap(),
            O2mResponse::Ack
        )
    };
}
