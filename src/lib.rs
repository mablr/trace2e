pub mod p2m {
    tonic::include_proto!("p2m");
    pub const FILE_DESCRIPTOR_SET: &[u8] = include_bytes!("../target/trace2e_descriptor.bin");
}
