#[derive(Debug, Clone)]
pub enum TraceabilityRequest {
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

pub enum ResourceRequest {
    ReadRequest,
    WriteRequest,
    ReadReport,
    WriteReport,
}

pub enum ResourceResponse {
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
