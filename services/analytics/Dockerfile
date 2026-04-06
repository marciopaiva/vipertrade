ARG RUST_VERSION=1.83
ARG RUST_BUILDER_IMAGE=rust:${RUST_VERSION}-slim-bookworm
ARG RUST_RUNTIME_IMAGE=debian:bookworm-slim
FROM ${RUST_BUILDER_IMAGE} AS builder

WORKDIR /usr/src/app

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY . .

RUN cargo build --release --package viper-analytics

FROM ${RUST_RUNTIME_IMAGE}

WORKDIR /app

RUN apt-get update && apt-get install -y ca-certificates libssl3 curl netcat-openbsd && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/app/target/release/viper-analytics /usr/local/bin/viper-analytics
COPY scripts/docker/wait-for-deps.sh /usr/local/bin/wait-for-deps.sh
RUN chmod +x /usr/local/bin/wait-for-deps.sh

RUN useradd -m -u 1000 -U viper
USER viper

CMD ["wait-for-deps.sh", "postgres:5432", "--", "viper-analytics"]
