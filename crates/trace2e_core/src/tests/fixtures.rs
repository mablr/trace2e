use crate::traceability::infrastructure::naming::{Fd, LocalizedResource, Resource};

pub(super) struct FileMapping {
    pid: i32,
    fd: i32,
    node_id: String,
    process: Resource,
    file: Resource,
}

pub(super) struct StreamMapping {
    pid: i32,
    fd: i32,
    node_id: String,
    process: Resource,
    stream: Resource,
}

impl FileMapping {
    pub fn new(pid: i32, fd: i32, path: &str) -> Self {
        Self::with_node_id(pid, fd, path, String::new())
    }

    pub fn with_node_id(pid: i32, fd: i32, path: &str, node_id: String) -> Self {
        Self {
            pid,
            fd,
            node_id,
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
            Resource::Fd(Fd::File(file)) => file.path.to_owned(),
            _ => panic!("FileMapping is not a file"),
        }
    }
    pub fn process(&self) -> Resource {
        self.process.to_owned()
    }
    pub fn file(&self) -> Resource {
        self.file.to_owned()
    }

    /// Get the localized process resource
    pub fn localized_process(&self) -> LocalizedResource {
        LocalizedResource::new(self.node_id.clone(), self.process.to_owned())
    }

    /// Get the localized file resource
    pub fn localized_file(&self) -> LocalizedResource {
        LocalizedResource::new(self.node_id.clone(), self.file.to_owned())
    }
}

impl StreamMapping {
    pub fn new(pid: i32, fd: i32, local_socket: &str, peer_socket: &str) -> Self {
        Self::with_node_id(pid, fd, local_socket, peer_socket, String::new())
    }

    pub fn with_node_id(
        pid: i32,
        fd: i32,
        local_socket: &str,
        peer_socket: &str,
        node_id: String,
    ) -> Self {
        Self {
            pid,
            fd,
            node_id,
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
            Resource::Fd(Fd::Stream(stream)) => stream.local_socket.to_owned(),
            _ => panic!("StreamMapping is not a stream"),
        }
    }
    pub fn stream_peer_socket(&self) -> String {
        match &self.stream {
            Resource::Fd(Fd::Stream(stream)) => stream.peer_socket.to_owned(),
            _ => panic!("StreamMapping is not a stream"),
        }
    }
    pub fn process(&self) -> Resource {
        self.process.to_owned()
    }
    pub fn stream(&self) -> Resource {
        self.stream.to_owned()
    }

    /// Get the localized process resource
    pub fn localized_process(&self) -> LocalizedResource {
        LocalizedResource::new(self.node_id.clone(), self.process.to_owned())
    }

    /// Get the localized stream resource
    pub fn localized_stream(&self) -> LocalizedResource {
        LocalizedResource::new(self.node_id.clone(), self.stream.to_owned())
    }
}

macro_rules! local_enroll {
    ($p2m:expr, $mapping:expr) => {
        assert_eq!(
            $p2m.call(crate::traceability::api::P2mRequest::LocalEnroll {
                pid: $mapping.pid(),
                fd: $mapping.fd(),
                path: $mapping.file_path(),
            })
            .await
            .unwrap(),
            crate::traceability::api::P2mResponse::Ack
        )
    };
}

macro_rules! remote_enroll {
    ($p2m:expr, $mapping:expr) => {
        assert_eq!(
            $p2m.call(crate::traceability::api::P2mRequest::RemoteEnroll {
                pid: $mapping.pid(),
                fd: $mapping.fd(),
                local_socket: $mapping.stream_local_socket(),
                peer_socket: $mapping.stream_peer_socket(),
            })
            .await
            .unwrap(),
            crate::traceability::api::P2mResponse::Ack
        )
    };
}

macro_rules! write_request {
    ($p2m:expr, $mapping:expr) => {
        match $p2m
            .call(crate::traceability::api::P2mRequest::IoRequest {
                pid: $mapping.pid(),
                fd: $mapping.fd(),
                output: true,
            })
            .await
        {
            Ok(crate::traceability::api::P2mResponse::Grant(flow_id)) => flow_id,
            _ => u128::MAX, // This means there was a policy violation or an error
        }
    };
}

macro_rules! read_request {
    ($p2m:expr, $mapping:expr) => {
        match $p2m
            .call(crate::traceability::api::P2mRequest::IoRequest {
                pid: $mapping.pid(),
                fd: $mapping.fd(),
                output: false,
            })
            .await
        {
            Ok(crate::traceability::api::P2mResponse::Grant(flow_id)) => flow_id,
            _ => u128::MAX, // This means there was a policy violation or an error
        }
    };
}

macro_rules! io_report {
    ($p2m:expr, $mapping:expr, $flow_id:expr, $result:expr) => {
        // If flow_id is u128::MAX, it means there was a policy violation or an error, do not report
        // flow_id
        assert_ne!($flow_id, u128::MAX);
        assert_eq!(
            $p2m.call(crate::traceability::api::P2mRequest::IoReport {
                pid: $mapping.pid(),
                fd: $mapping.fd(),
                grant_id: $flow_id,
                result: $result,
            })
            .await
            .unwrap(),
            crate::traceability::api::P2mResponse::Ack
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
            $o2m.call(crate::traceability::api::O2mRequest::GetReferences($resource))
                .await
                .unwrap(),
            crate::traceability::api::O2mResponse::References($provenance)
        )
    };
}

macro_rules! assert_policies {
    ($o2m:expr, $resource_set:expr, $policy_set:expr) => {
        assert_eq!(
            $o2m.call(crate::traceability::api::O2mRequest::GetPolicies($resource_set))
                .await
                .unwrap(),
            crate::traceability::api::O2mResponse::Policies($policy_set)
        )
    };
}

macro_rules! set_confidentiality {
    ($o2m:expr, $resource:expr, $confidentiality:expr) => {
        assert_eq!(
            $o2m.call(crate::traceability::api::O2mRequest::SetConfidentiality {
                resource: $resource,
                confidentiality: $confidentiality,
            })
            .await
            .unwrap(),
            crate::traceability::api::O2mResponse::Ack
        )
    };
}

macro_rules! set_integrity {
    ($o2m:expr, $resource:expr, $integrity:expr) => {
        assert_eq!(
            $o2m.call(crate::traceability::api::O2mRequest::SetIntegrity {
                resource: $resource,
                integrity: $integrity
            })
            .await
            .unwrap(),
            crate::traceability::api::O2mResponse::Ack
        )
    };
}

macro_rules! set_deleted {
    ($o2m:expr, $resource:expr) => {
        assert_eq!(
            $o2m.call(crate::traceability::api::O2mRequest::SetDeleted($resource)).await.unwrap(),
            crate::traceability::api::O2mResponse::Ack
        )
    };
}

#[allow(unused_macros)]
macro_rules! broadcast_deletion {
    ($o2m:expr, $resource:expr) => {
        assert_eq!(
            $o2m.call(crate::traceability::api::O2mRequest::BroadcastDeletion($resource))
                .await
                .unwrap(),
            crate::traceability::api::O2mResponse::Ack
        )
    };
}

#[allow(unused_macros)]
macro_rules! enforce_consent {
    ($o2m:expr, $resource:expr) => {
        assert_eq!(
            $o2m.call(crate::traceability::api::O2mRequest::EnforceConsent($resource))
                .await
                .unwrap(),
            crate::traceability::api::O2mResponse::Ack
        )
    };
}

#[allow(unused_macros)]
macro_rules! set_consent_decision {
    ($o2m:expr, $resource:expr, $consent:expr) => {
        assert_eq!(
            $o2m.call(crate::traceability::api::O2mRequest::SetConsentDecision($resource))
                .await
                .unwrap(),
            crate::traceability::api::O2mResponse::Ack
        )
    };
}
