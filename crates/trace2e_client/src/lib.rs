pub mod p2m;

#[cfg(feature = "o2m")]
pub mod o2m;

// re-export primitives for easier access
pub use trace2e_core::transport::grpc::proto::primitives;
