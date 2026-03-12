ARG RUST_VERSION=1.83
FROM rust:${RUST_VERSION}-slim-bookworm

ENV PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1
ENV RUSTUP_SKIP_UPDATE_CHECK=1

RUN rustup set profile minimal \
  && rustup toolchain install ${RUST_VERSION} --profile minimal --component rustfmt,clippy \
  && rustup default ${RUST_VERSION}

RUN apt-get update && apt-get install -y --no-install-recommends \
  pkg-config \
  libssl-dev \
  python3 \
  python3-dev \
  ca-certificates \
  curl \
  && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app
