fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .file_descriptor_set_path("trace2e_descriptor.bin")
        .compile(&["../proto/trace2e.proto"], &["../proto"])?;
    Ok(())
}
