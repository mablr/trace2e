FROM rust:latest AS builder

# Install protobuf-compiler and other necessary tools
RUN apt update && apt install -y protobuf-compiler libprotobuf-dev iputils-ping && rm -rf /var/lib/apt/lists/*

WORKDIR /trace2e

# Copy the source code into the container
COPY proto/ proto/
COPY trace2e_middleware/ trace2e_middleware/
COPY trace2e_client/ trace2e_client/
COPY stde2e/ stde2e/
COPY Cargo.toml ./
COPY patches/ patches/

# Build the project
RUN cargo build --release

# Prepare Tokio
WORKDIR /tokioe2e
RUN git clone --branch tokio-1.41.1 https://github.com/tokio-rs/tokio .
RUN git apply ../trace2e/patches/tokioe2e.patch

# Clone Hyper
WORKDIR /hyper
RUN git clone --branch v1.5.1 https://github.com/hyperium/hyper .

RUN sed -i "s/1337/1338/" examples/send_file.rs
RUN sed -i "s/3000/1338/;s/\[127, 0, 0, 1\], 3001/\[0, 0, 0, 0\], 3002/" examples/gateway.rs
RUN cargo build --example send_file --features full
RUN cargo build --example gateway --features full
RUN mv /hyper/target/debug/examples/send_file /hyper/target/debug/examples/send_file_vanilla
RUN mv /hyper/target/debug/examples/gateway /hyper/target/debug/examples/gateway_vanilla

RUN git checkout -f
RUN git apply ../trace2e/patches/hyper_example_gateway.patch
RUN git apply ../trace2e/patches/hyper_tokio.patch
RUN cargo build --example send_file --features full
RUN cargo build --example gateway --features full


# Final stage to set up runtime environment
FROM debian:bookworm-slim

# Copy the compiled binaries from the builder stage
COPY --from=builder /trace2e/target/release/trace2e_middleware /trace2e_middleware
COPY --from=builder /hyper/target/debug/examples/send_file_vanilla /hyper_server
COPY --from=builder /hyper/target/debug/examples/send_file /hypere2e_server
COPY --from=builder /hyper/examples/send_file_index.html /examples/send_file_index.html
COPY --from=builder /hyper/target/debug/examples/gateway_vanilla /hyper_gateway
COPY --from=builder /hyper/target/debug/examples/gateway /hypere2e_gateway

# Ensure the binaries are executable
RUN chmod +x /trace2e_middleware
RUN chmod +x /hyper*
