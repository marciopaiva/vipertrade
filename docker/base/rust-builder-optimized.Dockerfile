# Dockerfile otimizado para cache de dependências Cargo
# Uso: docker build -f rust-builder-optimized.Dockerfile -t vipertrade-base-rust-builder-optimized:1.83 .

ARG RUST_VERSION=1.83
FROM rust:${RUST_VERSION}-slim-bookworm

ENV CARGO_HOME=/usr/local/cargo \
  RUSTUP_HOME=/usr/local/rustup \
  PATH=/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin \
  RUSTUP_SKIP_UPDATE_CHECK=1 \
  PYO3_PYTHON=/usr/bin/python3 \
  PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 \
  CARGO_NET_GIT_FETCH_WITH_CLI=true

# Instalar dependências do sistema
RUN apt-get update && apt-get install -y --no-install-recommends \
  pkg-config \
  libssl-dev \
  python3 \
  python3-dev \
  ca-certificates \
  curl \
  git \
  && rm -rf /var/lib/apt/lists/*

# Configurar Rust
RUN rustup set profile minimal \
  && rustup toolchain install ${RUST_VERSION} --profile minimal --component rustfmt,clippy \
  && rustup default ${RUST_VERSION} \
  && ln -sf /usr/local/cargo/bin/cargo /usr/local/bin/cargo \
  && ln -sf /usr/local/cargo/bin/rustc /usr/local/bin/rustc \
  && ln -sf /usr/local/cargo/bin/rustfmt /usr/local/bin/rustfmt \
  && ln -sf /usr/local/cargo/bin/cargo-clippy /usr/local/bin/cargo-clippy

WORKDIR /work

# ═══════════════════════════════════════════════════════════════════════════
# CAMADA 1: Cache do Cargo.lock + Cargo.toml
# Esta camada muda pouco, então o cache é preservado na maioria dos builds
# ═══════════════════════════════════════════════════════════════════════════

COPY Cargo.toml Cargo.lock ./

# Apenas fetch das dependências (download sem compilar)
# Isso é cacheado e evita download repetido
RUN cargo fetch --locked

# ═══════════════════════════════════════════════════════════════════════════
# CAMADA 2: Código fonte
# Esta camada muda frequentemente
# ═══════════════════════════════════════════════════════════════════════════

COPY . .

# Build e testes (usa cache do cargo fetch)
# Não fazemos build aqui pois o código muda frequentemente
# O build será feito no runtime do container

CMD ["bash"]
