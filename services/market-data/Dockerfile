# Base Rust Image
ARG RUST_VERSION=1.83
ARG RUST_BUILDER_IMAGE=rust:${RUST_VERSION}-slim-bookworm
ARG RUST_RUNTIME_IMAGE=debian:bookworm-slim
FROM ${RUST_BUILDER_IMAGE} AS builder

WORKDIR /usr/src/app

# Install build dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy workspace manifest
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY . .

# Build specific package
RUN cargo build --release --package viper-market-data

# Runtime Image
FROM ${RUST_RUNTIME_IMAGE}

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y ca-certificates libssl3 curl && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /usr/src/app/target/release/viper-market-data /usr/local/bin/viper-market-data

# Create non-root user
RUN useradd -m -u 1000 -U viper
USER viper

CMD ["viper-market-data"]
