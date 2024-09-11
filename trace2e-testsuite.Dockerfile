# (in rust project root) BUILD_COMMAND:
# $ docker build -t trace2e-interactive -f trace2e-interactive.Dockerfile .

FROM rust:latest AS builder

# Install protobuf-compiler and other necessary tools
RUN apt update && apt install -y protobuf-compiler libprotobuf-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /trace2e

# Copy the source code into the container
COPY proto/ proto/
COPY src/ src/
COPY tests/ tests/
COPY build.rs Cargo.toml tests.sh ./

# Set the entrypoint
ENTRYPOINT ["./tests.sh"]
