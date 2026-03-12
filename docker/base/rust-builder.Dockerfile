ARG RUST_VERSION=1.83
FROM rust:${RUST_VERSION}-slim-bookworm

RUN rustup set profile minimal \
  && rustup toolchain install ${RUST_VERSION} --profile minimal --component rustfmt,clippy \
  && rustup default ${RUST_VERSION}

ENV RUSTUP_SKIP_UPDATE_CHECK=1

RUN apt-get update && apt-get install -y --no-install-recommends \
  pkg-config \
  libssl-dev \
  ca-certificates \
  curl \
  && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app
