# (in rust project root) BUILD_COMMAND:
# $ docker build -t trace2e -f trace2e.Dockerfile .

FROM rust:latest AS builder

# Install protobuf-compiler and other necessary tools
RUN apt update && apt install -y protobuf-compiler libprotobuf-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /trace2e

# Copy the source code into the container
COPY proto/ proto/
COPY src/ src/
COPY tests/ tests/
COPY build.rs Cargo.toml tests.sh ./

# Build the project
RUN cargo build --release --features verbose

# Use a minimal image for the runtime
FROM debian:bookworm-slim

# Copy the compiled binary from the build stage
COPY --from=builder /trace2e/target/release/trace2e /usr/local/bin/trace2e

# Set the entrypoint
ENTRYPOINT ["/usr/local/bin/trace2e"]
