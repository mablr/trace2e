fn main () -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .file_descriptor_set_path("target/p2m_descriptor.bin")
        .compile(&["./proto/p2m.proto"], &["./proto"])?;
    Ok(())
}