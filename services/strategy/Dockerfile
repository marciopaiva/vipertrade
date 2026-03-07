# Base Rust Image
ARG RUST_VERSION=1.83
FROM rust:${RUST_VERSION}-slim-bookworm AS builder

WORKDIR /usr/src/app

ENV PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1

# Install build dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy workspace manifest
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY . .

# Build specific package
RUN cargo build --release --package viper-strategy

# Runtime Image
FROM python:3.12-slim-bookworm

ENV LD_LIBRARY_PATH=/usr/local/lib:$LD_LIBRARY_PATH

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y ca-certificates libssl3 curl && rm -rf /var/lib/apt/lists/*

# Install Tupã CLI
ARG TUPA_VERSION=v0.8.0
ADD https://github.com/marciopaiva/tupalang/releases/download/${TUPA_VERSION}/tupa-linux-x86_64 /usr/local/bin/tupa
RUN chmod +x /usr/local/bin/tupa &&     ls -l /usr/local/bin/tupa

# Copy binary from builder
COPY --from=builder /usr/src/app/target/release/viper-strategy /usr/local/bin/viper-strategy

# Create non-root user
RUN useradd -m -u 1000 -U viper
USER viper

CMD ["viper-strategy"]
