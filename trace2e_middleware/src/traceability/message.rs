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
