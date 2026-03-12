ARG RUST_VERSION=1.83
FROM rust:${RUST_VERSION}-slim-bookworm AS builder

WORKDIR /usr/src/app

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY . .

RUN cargo build --release --package viper-analytics

FROM debian:bookworm-slim

WORKDIR /app

RUN apt-get update && apt-get install -y ca-certificates libssl3 curl && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/app/target/release/viper-analytics /usr/local/bin/viper-analytics

RUN useradd -m -u 1000 -U viper
USER viper

CMD ["viper-analytics"]
