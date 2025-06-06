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
        id: u128,
        success: bool,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum P2mResponse {
    Grant(u128), // <- P2mRequest::IoRequest
    Ack,         // <- P2mRequest::{LocalEnroll, RemoteEnroll, Report}
}

#[derive(Debug, Clone)]
pub enum TraceabilityRequest {
    InitResource(Identifier), // -> TraceabilityResponse::Ack
    Request {
        // -> TraceabilityResponse::Grant
        source: Identifier,
        destination: Identifier,
    },
    Report {
        // -> TraceabilityResponse::Ack
        source: Identifier,
        destination: Identifier,
        success: bool,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TraceabilityResponse {
    Grant,
    Ack,
    Wait, // Only produced by SequencerService and mapped to Ack by WaitingQueueService
}
