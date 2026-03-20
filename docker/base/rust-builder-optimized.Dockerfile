# Dockerfile otimizado para cache de dependências Cargo
# Uso: docker build -f rust-builder-optimized.Dockerfile -t vipertrade-base-rust-builder:1.83 .

ARG RUST_VERSION=1.83
FROM rust:${RUST_VERSION}-slim-bookworm AS base

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
# CAMADA 1: Cache do Cargo.lock e workspace Cargo.toml
# Isso muda pouco, então o cache é preservado na maioria dos builds
# ═══════════════════════════════════════════════════════════════════════════

COPY Cargo.lock Cargo.toml ./

# Criar crates e services dummy para pré-build das dependências
# Isso força o Cargo a baixar todas as deps sem o código fonte real
RUN mkdir -p crates/viper-domain/src \
           services/market-data/src \
           services/analytics/src \
           services/strategy/src \
           services/executor/src \
           services/monitor/src \
           services/backtest/src \
           services/api/src \
           services/web/src

# Copiar apenas Cargo.toml de cada serviço
COPY crates/viper-domain/Cargo.toml crates/viper-domain/
COPY services/market-data/Cargo.toml services/market-data/
COPY services/analytics/Cargo.toml services/analytics/
COPY services/strategy/Cargo.toml services/strategy/
COPY services/executor/Cargo.toml services/executor/
COPY services/monitor/Cargo.toml services/monitor/
COPY services/backtest/Cargo.toml services/backtest/
COPY services/api/Cargo.toml services/api/
COPY services/web/Cargo.toml services/web/

# Criar main.rs dummy em cada serviço
RUN echo 'fn main() {}' > crates/viper-domain/src/lib.rs && \
    for svc in market-data analytics strategy executor monitor backtest api web; do \
      echo 'fn main() {}' > services/$$svc/src/main.rs; \
    done

# Pré-build das dependências (isso é cacheado!)
# Usa --message-format=short para output mais limpo
RUN cargo fetch --locked && \
    cargo build --workspace --locked --message-format=short 2>&1 | tail -20

# Limpar bins intermediários mas manter deps compiladas
RUN cargo clean --release 2>/dev/null || true

# ═══════════════════════════════════════════════════════════════════════════
# CAMADA 2: Código fonte real
# Esta camada muda frequentemente, mas as deps já estão em cache
# ═══════════════════════════════════════════════════════════════════════════

# Agora copiar o código fonte real
COPY . .

# Build final (usa cache das dependências)
RUN cargo build --workspace --locked --message-format=short 2>&1 | tail -20

CMD ["bash"]
