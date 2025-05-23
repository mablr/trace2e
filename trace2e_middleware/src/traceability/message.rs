#[derive(Debug, Clone, Copy, Eq, PartialEq)]
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
