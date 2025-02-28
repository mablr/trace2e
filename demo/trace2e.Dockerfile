# Prepare Hyper source code
FROM rust:latest AS hyper_source
WORKDIR /hyper
RUN git clone --branch v1.5.1 https://github.com/hyperium/hyper .

FROM rust:latest AS protobuf_base
RUN apt update && apt install -y protobuf-compiler libprotobuf-dev && rm -rf /var/lib/apt/lists/*


# Prepare trace2e middleware
FROM protobuf_base AS trace2e_middleware
COPY trace2e_middleware/ trace2e_middleware/
COPY proto/ proto/
WORKDIR /trace2e_middleware
RUN cargo build --release

# Prepare stde2e examples
FROM protobuf_base AS stde2e
COPY trace2e_client/ trace2e_client/
COPY proto/p2m_api.proto proto/p2m_api.proto
COPY stde2e/ stde2e/
WORKDIR /stde2e
RUN ls examples | sed "s/.rs$//g" | xargs -I % cargo build --example %

# Get Tokio source code and patch it
FROM protobuf_base AS hypere2e_source
WORKDIR /trace2e
COPY patches/ patches/
COPY trace2e_client/ trace2e_client/
COPY proto/p2m_api.proto proto/p2m_api.proto
WORKDIR /tokioe2e
RUN git clone --branch tokio-1.41.1 https://github.com/tokio-rs/tokio .
RUN git apply ../trace2e/patches/tokioe2e.patch
WORKDIR /hyper
RUN git clone --branch v1.5.1 https://github.com/hyperium/hyper .
RUN git apply ../trace2e/patches/hyper_tokio.patch

# Build Hyper client vanilla
FROM hyper_source AS hyper_client
WORKDIR /hyper
RUN cargo build --example client --features full

# Build Hyper client for trace2e
FROM hypere2e_source AS hypere2e_client
WORKDIR /hyper
RUN cargo build --example client --features full

# Build Hyper server vanilla
FROM hyper_source AS hyper_server
WORKDIR /hyper
RUN sed -i "s/1337/1338/" examples/send_file.rs
RUN sed -i "s/1337/1338/" examples/send_file.rs
RUN sed -i "s/3000/1338/;s/\[127, 0, 0, 1\], 3001/\[0, 0, 0, 0\], 3002/" examples/gateway.rs
RUN cargo build --example send_file --features full
RUN cargo build --example gateway --features full

# Build Hyper server for trace2e
FROM hypere2e_source AS hypere2e_server
WORKDIR /hyper
RUN git apply ../trace2e/patches/hyper_example_gateway.patch
RUN cargo build --example send_file --features full
RUN cargo build --example gateway --features full

# Install tools for interactive runtime
FROM debian:bookworm-slim AS interactive_runtime
RUN apt update && apt install -y iputils-ping iproute2 wget && rm -rf /var/lib/apt/lists/*
RUN wget https://github.com/fullstorydev/grpcurl/releases/download/v1.9.2/grpcurl_1.9.2_linux_amd64.deb
RUN dpkg -i grpcurl_1.9.2_linux_amd64.deb
RUN rm grpcurl_1.9.2_linux_amd64.deb

# Create stde2e runtime environment
FROM interactive_runtime AS stde2e_runtime
COPY --from=trace2e_middleware /trace2e_middleware/target/release/trace2e_middleware /trace2e_middleware
COPY --from=stde2e /stde2e/target/debug/examples/tcp_client /tcp_client
COPY --from=stde2e /stde2e/target/debug/examples/tcp_client_e2e /tcp_client_e2e
COPY --from=stde2e /stde2e/target/debug/examples/tcp_server /tcp_server
COPY --from=stde2e /stde2e/target/debug/examples/tcp_server_e2e /tcp_server_e2e
COPY --from=stde2e /stde2e/target/debug/examples/file_forwarder /file_forwarder
COPY --from=stde2e /stde2e/target/debug/examples/file_forwarder_e2e /file_forwarder_e2e

# Create Hyper client runtime environment
FROM interactive_runtime AS hyper_client_runtime
# Copy the compiled binaries from the builder stage
COPY --from=trace2e_middleware /trace2e_middleware/target/release/trace2e_middleware /trace2e_middleware
COPY --from=hyper_client /hyper/target/debug/examples/client /hyper_client
COPY --from=hypere2e_client /hyper/target/debug/examples/client /hypere2e_client


# Create Hyper server runtime environment
FROM debian:bookworm-slim AS hyper_server_runtime
COPY --from=trace2e_middleware /trace2e_middleware/target/release/trace2e_middleware /trace2e_middleware
COPY --from=hyper_server /hyper/target/debug/examples/send_file /hyper_server
COPY --from=hypere2e_server /hyper/target/debug/examples/send_file /hypere2e_server
COPY --from=hyper_server /hyper/target/debug/examples/gateway /hyper_gateway
COPY --from=hypere2e_server /hyper/target/debug/examples/gateway /hypere2e_gateway
COPY --from=hyper_server /hyper/examples/send_file_index.html /examples/send_file_index.html
