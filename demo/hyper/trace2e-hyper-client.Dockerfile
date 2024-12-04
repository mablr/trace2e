FROM rust:latest AS builder

# Install protobuf-compiler and other necessary tools
RUN apt update && apt install -y protobuf-compiler libprotobuf-dev iputils-ping wget && rm -rf /var/lib/apt/lists/*

# Install grpcurl
RUN wget https://github.com/fullstorydev/grpcurl/releases/download/v1.9.2/grpcurl_1.9.2_linux_amd64.deb
RUN dpkg -i grpcurl_1.9.2_linux_amd64.deb
RUN rm grpcurl_1.9.2_linux_amd64.deb

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
RUN git apply ../trace2e/patches/hyper_tokio.patch
RUN cargo build --example client --features full
