fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .file_descriptor_set_path("target/trace2e_api_descriptor.bin")
        .compile(&["./proto/trace2e_api.proto"], &["./proto"])?;
    Ok(())
}
