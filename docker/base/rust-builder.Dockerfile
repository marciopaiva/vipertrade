ARG RUST_VERSION=1.83
FROM rust:${RUST_VERSION}-slim-bookworm

ENV CARGO_HOME=/usr/local/cargo \
  RUSTUP_HOME=/usr/local/rustup \
  PATH=/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin \
  RUSTUP_SKIP_UPDATE_CHECK=1 \
  PYO3_PYTHON=/usr/bin/python3 \
  PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1

RUN rustup set profile minimal \
  && rustup toolchain install ${RUST_VERSION} --profile minimal --component rustfmt,clippy \
  && rustup default ${RUST_VERSION} \
  && ln -sf /usr/local/cargo/bin/cargo /usr/local/bin/cargo \
  && ln -sf /usr/local/cargo/bin/rustc /usr/local/bin/rustc \
  && ln -sf /usr/local/cargo/bin/rustfmt /usr/local/bin/rustfmt \
  && ln -sf /usr/local/cargo/bin/cargo-clippy /usr/local/bin/cargo-clippy

RUN apt-get update && apt-get install -y --no-install-recommends \
  pkg-config \
  libssl-dev \
  python3 \
  python3-dev \
  ca-certificates \
  curl \
  && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app
