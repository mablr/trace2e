fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .file_descriptor_set_path("p2m_descriptor.bin")
        .compile(&["../proto/p2m_api.proto"], &["../proto"])?;
    Ok(())
}
