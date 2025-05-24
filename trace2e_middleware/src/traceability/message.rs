#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TraceabilityRequest {
    ReserveShared,
    ReserveExclusive,
    ReleaseShared,
    ReleaseExclusive,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TraceabilityResponse {
    Reserved,
    Released,
}
