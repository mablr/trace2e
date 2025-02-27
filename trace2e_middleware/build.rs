fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .file_descriptor_set_path("p2m_descriptor.bin")
        .compile(&["../proto/p2m_api.proto"], &["../proto"])?;
    tonic_build::configure()
        .file_descriptor_set_path("m2m_descriptor.bin")
        .compile(&["../proto/m2m_api.proto"], &["../proto"])?;
    tonic_build::configure()
        .file_descriptor_set_path("user_descriptor.bin")
        .compile(&["../proto/user_api.proto"], &["../proto"])?;
    Ok(())
}
