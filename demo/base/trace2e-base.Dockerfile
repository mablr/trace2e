FROM rust:latest AS trace2e_builder

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