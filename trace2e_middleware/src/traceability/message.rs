use super::naming::Identifier;

#[derive(Debug, Clone)]
pub enum P2mRequest {
    LocalEnroll {
        pid: i32,
        fd: i32,
        path: String,
    },
    RemoteEnroll {
        pid: i32,
        fd: i32,
        local_socket: String,
        peer_socket: String,
    },
    IoRequest {
        pid: i32,
        fd: i32,
        output: bool,
    },
    IoReport {
        id: usize,
        success: bool,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TraceabilityResponse {
    Grant(usize),
    Ack,
}

#[derive(Debug, Clone)]
pub enum FlowRequest {
    Request {
        process: Identifier,
        fd: Identifier,
        output: bool,
    },
    Report {
        id: usize,
        success: bool,
    },
}

/// The requests are expecting compliance labels as response
/// The reports may lead to an update of the compliance labels
pub enum ResourceRequest {
    ReadRequest,
    WriteRequest,
    ReadReport,
    WriteReport,
}

pub enum ResourceResponse { // TODO : Refactor this to convey Compliance Labels
    Grant,
    Ack,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ReservationRequest {
    GetShared,
    GetExclusive,
    ReleaseShared,
    ReleaseExclusive,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ReservationResponse {
    Reserved,
    Released,
}
