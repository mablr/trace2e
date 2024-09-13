fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .file_descriptor_set_path("target/p2m_descriptor.bin")
        .compile(&["./proto/p2m_api.proto"], &["./proto"])?;
    tonic_build::configure()
        .file_descriptor_set_path("target/m2m_descriptor.bin")
        .compile(&["./proto/m2m_api.proto"], &["./proto"])?;
    Ok(())
}
