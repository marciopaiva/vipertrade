# Unified ViperTrade image — builds the single `viper` binary that runs any
# service by role (VIPER_ROLE env). Runtime base covers all roles (the strategy
# role needs the python/cargo-tupa runtime).
ARG RUST_VERSION=1.83
ARG BUILDER_IMAGE=rust:${RUST_VERSION}-slim-bookworm
ARG RUNTIME_IMAGE=python:3.12-slim-bookworm

FROM ${BUILDER_IMAGE} AS builder
WORKDIR /usr/src/app
ENV PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1
RUN apt-get update && apt-get install -y pkg-config libssl-dev python3 python3-dev && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY . .
RUN cargo build --release --package viper

FROM ${RUNTIME_IMAGE}
ENV LD_LIBRARY_PATH=/usr/local/lib:$LD_LIBRARY_PATH
WORKDIR /app
RUN apt-get update && apt-get install -y ca-certificates libssl3 curl libpython3.11 netcat-openbsd && rm -rf /var/lib/apt/lists/*

ARG TUPA_VERSION=v0.10.0
ADD https://github.com/marciopaiva/tupalang/releases/download/${TUPA_VERSION}/cargo-tupa-linux-x86_64 /usr/local/bin/tupa
RUN chmod +x /usr/local/bin/tupa && ln -sf /usr/local/bin/tupa /usr/local/bin/cargo-tupa

COPY --from=builder /usr/src/app/target/release/viper /usr/local/bin/viper
COPY --from=builder /usr/src/app/config /app/config
COPY scripts/docker/wait-for-deps.sh /usr/local/bin/wait-for-deps.sh
RUN chmod +x /usr/local/bin/wait-for-deps.sh

RUN useradd -m -u 1000 -U viper
USER viper

# Default: run viper (role from VIPER_ROLE env). k8s/compose override `command`
# to add per-role wait-for-deps before `viper`.
CMD ["viper"]
