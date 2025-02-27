FROM trace2e-base AS builder

# Prepare Hyper clients
WORKDIR /hyper
RUN git clone --branch v1.5.1 https://github.com/hyperium/hyper .
RUN cargo build --example client --features full
RUN mv /hyper/target/debug/examples/client /hyper/target/debug/examples/client_vanilla
RUN git apply ../trace2e/patches/hyper_tokio.patch
RUN cargo build --example client --features full

# Final stage to set up runtime environment
FROM debian:bookworm-slim

# Copy the compiled binaries from the builder stage
COPY --from=builder /trace2e/target/release/trace2e_middleware /trace2e_middleware
COPY --from=builder /hyper/target/debug/examples/client_vanilla /hyper_client
COPY --from=builder /hyper/target/debug/examples/client /hypere2e_client

# Ensure the binaries are executable
RUN chmod +x /trace2e_middleware
RUN chmod +x /hyper_client

# Install ping and wget
RUN apt update && apt install -y iputils-ping wget&& rm -rf /var/lib/apt/lists/*

# Install grpcurl
RUN wget https://github.com/fullstorydev/grpcurl/releases/download/v1.9.2/grpcurl_1.9.2_linux_amd64.deb
RUN dpkg -i grpcurl_1.9.2_linux_amd64.deb
RUN rm grpcurl_1.9.2_linux_amd64.deb