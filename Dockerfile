# Multi-stage build for efficient container
FROM rust:1.91-slim AS builder

# Install protobuf compiler
RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY proto ./proto
COPY patches ./patches
COPY rustfmt.toml ./

# Build release binaries
RUN cargo build --release -p trace2e_middleware
RUN cargo build --release -p trace2e_interactive
RUN cargo build --release -p trace2e_interactive --bin std-proc --no-default-features
RUN cargo build --release -p trace2e_interactive --bin std-op --no-default-features

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    iproute2 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binaries from builder
COPY --from=builder /build/target/release/trace2e_middleware /app/
COPY --from=builder /build/target/release/e2e-proc /app/
COPY --from=builder /build/target/release/e2e-op /app/
COPY --from=builder /build/target/release/std-proc /app/
COPY --from=builder /build/target/release/std-op /app/

# Create directory for playbooks and data
RUN mkdir -p /app/playbooks /app/data

# Default command (override in docker-compose)
CMD ["/app/trace2e_middleware"]
