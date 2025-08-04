use crate::traceability::naming::{File, Stream};

pub(super) struct FileMapping {
    pub pid: i32,
    pub fd: i32,
    pub resource: File,
}

pub(super) struct StreamMapping {
    pub pid: i32,
    pub fd: i32,
    pub resource: Stream,
}

impl FileMapping {
    pub fn new(pid: i32, fd: i32, path: &str) -> Self {
        Self {
            pid,
            fd,
            resource: File {
                path: path.to_string(),
            },
        }
    }
}

impl StreamMapping {
    pub fn new(pid: i32, fd: i32, local_socket: &str, peer_socket: &str) -> Self {
        Self {
            pid,
            fd,
            resource: Stream {
                local_socket: local_socket.to_string(),
                peer_socket: peer_socket.to_string(),
            },
        }
    }
}

macro_rules! local_enroll {
    ($p2m:expr, $mapping:expr) => {
        assert_eq!(
            $p2m.call(P2mRequest::LocalEnroll {
                pid: $mapping.pid,
                fd: $mapping.fd,
                path: $mapping.resource.path,
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
                pid: $mapping.pid,
                fd: $mapping.fd,
                local_socket: $mapping.resource.local_socket.to_string(),
                peer_socket: $mapping.resource.peer_socket.to_string(),
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
                pid: $mapping.pid,
                fd: $mapping.fd,
                output: true,
            })
            .await
            .unwrap()
        {
            P2mResponse::Grant(flow_id) => flow_id,
            _ => panic!("Expected P2mResponse::Grant"),
        }
    };
}

macro_rules! read_request {
    ($p2m:expr, $mapping:expr) => {
        match $p2m
            .call(P2mRequest::IoRequest {
                pid: $mapping.pid,
                fd: $mapping.fd,
                output: false,
            })
            .await
            .unwrap()
        {
            P2mResponse::Grant(flow_id) => flow_id,
            _ => panic!("Expected P2mResponse::Grant"),
        }
    };
}

macro_rules! io_report {
    ($p2m:expr, $mapping:expr, $flow_id:expr, $result:expr) => {
        assert_eq!(
            $p2m.call(P2mRequest::IoReport {
                pid: $mapping.pid,
                fd: $mapping.fd,
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
