mod proto {
    tonic::include_proto!("trace2e");
    pub mod primitives {
        tonic::include_proto!("trace2e.primitives");
    }
    pub mod messages {
        tonic::include_proto!("trace2e.messages");
    }
}

pub mod p2m;

#[cfg(feature = "o2m")]
pub mod o2m;

// Re-export commonly used types
pub mod primitives {
    pub use crate::proto::primitives::Flow;

    #[cfg(feature = "o2m")]
    pub use crate::proto::primitives::{
        Confidentiality, Destination, LocalizedResource, LocalizedResourceWithParent,
        MappedLocalizedPolicy, Policy, References, Resource,
    };
}

#[cfg(feature = "o2m")]
pub mod messages {
    pub use crate::proto::messages::ConsentNotification;
}
