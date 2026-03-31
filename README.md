# ViperTrade

<!-- markdownlint-disable MD033 -->
<p align="center">
  <img src="docs/assets/ViperTrade.png" alt="ViperTrade" width="260" />
</p>

<h1 align="center">ViperTrade</h1>

<p align="center"><strong>Auditable algorithmic trading with typed strategy policy and live operational telemetry.</strong></p>

<p align="center">Rust microservices, TupaLang-driven strategy policy, Docker runtime parity, and operator-first observability.</p>

<p align="center">
  <a href="https://github.com/marciopaiva/vipertrade/actions/workflows/ci.yml">
    <img
      alt="CI"
      src="https://img.shields.io/github/actions/workflow/status/marciopaiva/vipertrade/ci.yml?branch=main&label=CI"
    />
  </a>
  <img alt="Rust" src="https://img.shields.io/badge/Rust-1.83-black?logo=rust" />
  <img alt="TupaLang" src="https://img.shields.io/badge/TupaLang-typed%20strategy%20runtime-0ea5e9" />
  <img alt="Docker Compose" src="https://img.shields.io/badge/Docker-Compose-2496ED?logo=docker&logoColor=white" />
  <img alt="Modes" src="https://img.shields.io/badge/Modes-paper%20%7C%20testnet%20%7C%20mainnet-0f766e" />
</p>

<p align="center">
  <a href="docs/README.md">Docs</a> •
  <a href="docs/spec/README.md">Specs</a> •
  <a href="docs/releases/README.md">Releases</a> •
  <a href="https://github.com/marciopaiva/tupalang">TupaLang</a>
</p>
<!-- markdownlint-enable MD033 -->

---

ViperTrade is a production-oriented lead-trader runtime for Bybit copy trading.
It is built around deterministic strategy evaluation, replayable runtime behavior,
strong operational controls, and evidence-driven iteration.

Instead of hiding strategy logic inside application code, ViperTrade uses
[TupaLang](https://github.com/marciopaiva/tupalang) as a typed policy layer.
The Rust services handle live state, exchange interaction, persistence, and
operator tooling; the `.tp` strategy layer keeps decision semantics easier to
validate, review, and evolve.

## Why ViperTrade

Most trading systems fail in production for reasons that go beyond signal quality:

- strategy behavior becomes hard to audit
- runtime behavior drifts away from original design
- exchange and reconciliation edge cases break assumptions
- operators lack safe controls during incidents
- local validation does not match what CI or deployment actually run

ViperTrade exists to close that gap between strategy design and production execution.

## Architecture At A Glance

Core services:

- `market-data`
  - ingests and normalizes exchange signals
- `strategy`
  - loads the validated Tupa-derived runtime plan and emits decisions
- `executor`
  - translates decisions into paper, testnet, or mainnet actions
- `monitor`
  - checks drift, reconciliation, and service health
- `api`
  - exposes status, trades, positions, controls, and diagnostics
- `web`
  - operator dashboard with live runtime context
- `postgres` and `redis`
  - persistence, audit, and event transport

## Why TupaLang Matters Here

TupaLang is not used in this repository as a demo dependency. It is part of the applied architecture.

In ViperTrade, TupaLang helps by:

- separating strategy intent from runtime plumbing
- validating the strategy pipeline before runtime
- loading a checked execution plan in-process at service startup
- reducing hot-path strategy complexity in Rust
- making policy changes easier to review, audit, and explain

That gives us a cleaner split:

- TupaLang
  - strategy policy, typed contracts, explainable decisions
- Rust runtime
  - live market state, exchange execution, persistence, telemetry, controls

## Quickstart

```bash
git clone https://github.com/marciopaiva/vipertrade.git
cd vipertrade
sudo dnf install -y openssl-devel
cp compose/.env.example compose/.env
make build-base-images
./scripts/init-secrets.sh
./scripts/security-check.sh
make compose-up
make health
```

Open:

- Web dashboard: `http://localhost:3000`
- API: `http://localhost:8080`

## Daily Workflow

Start the stack:

```bash
make compose-up
```

Check service health:

```bash
make health
```

Run runtime validation:

```bash
make validate-runtime
```

Run local CI parity before commit or push:

```bash
make validate-ci
```

Reset the local paper database:

```bash
make data-reset-paper-db
```

Stop the stack:

```bash
make compose-down
```

## Runtime Modes

- `paper`
  - live market data with simulated wallet and positions in Postgres
- `testnet`
  - real exchange interaction on Bybit testnet
- `mainnet`
  - real exchange execution on Bybit mainnet

This keeps the operational model stable while the execution surface evolves.

## What The Platform Optimizes For

- deterministic service behavior
- operator-first runtime visibility
- health checks and kill-switch controls
- Docker-based runtime parity
- replayable validation and CI discipline
- audit-friendly decision history
- staged progression from paper to real execution

## Local Validation

Fast workspace checks:

```bash
make validate-workspace-quick
```

Full local validation:

```bash
make validate-full
```

CI-aligned local run:

```bash
make validate-ci
```

Direct host-side Rust validation on Fedora WSL:

```bash
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 cargo check --workspace --locked
```

Strict docs mode:

```bash
CI_LOCAL_STRICT_DOCS=1 ./scripts/ci-local.sh
```

## Builder-Based Rust Validation

After building the base images, you can validate inside the standard builder image:

```bash
docker run --rm \
  -e PYO3_PYTHON=/usr/bin/python3 \
  -v "$PWD":/work \
  -w /work \
  vipertrade-base-rust-builder:1.83 \
  cargo check --locked

docker run --rm \
  -e PYO3_PYTHON=/usr/bin/python3 \
  -v "$PWD":/work \
  -w /work \
  vipertrade-base-rust-builder:1.83 \
  cargo clippy --all-targets -- -D warnings
```

## Documentation

Documentation is organized by intent:

- `docs/spec/`
  - design and system specification
- `docs/operations/`
  - operator workflows and runtime evidence
- `docs/releases/`
  - release and change history
- `docs/legacy/`
  - archived material kept for reference

Start here:

- [Documentation Index](docs/README.md)
- [Spec Index](docs/spec/README.md)
- [Release Notes](docs/releases/README.md)

## Repository Interface

`make` is the main developer and operator interface.

Useful commands:

- `make health`
- `make validate-full`
- `make validate-workspace-quick`
- `make validate-ci`
- `make validate-runtime`
- `make build-base-images`
- `make compose-up`
- `make compose-down`
- `make compose-restart`
- `make compose-ps`
- `make compose-logs`
- `make data-reset-paper-db`
- `make control-kill-switch-status`
- `make control-kill-switch-enable`
- `make control-kill-switch-disable`

## Status

ViperTrade is being developed as an applied trading runtime with TupaLang as
its strategy-policy layer. Paper mode, diagnostics, audit trails, and local
operator tooling are active parts of the current workflow.

## License

MIT. See [LICENSE](LICENSE).
