# ViperTrade

<!-- markdownlint-disable MD033 -->
<p align="center">
  <img src="docs/assets/ViperTrade.png" alt="ViperTrade" width="280" />
</p>

<p align="center">
  <img alt="CI" src="https://img.shields.io/github/actions/workflow/status/marciopaiva/vipertrade/ci.yml?branch=main&label=CI" />
  <img alt="Tupa" src="https://img.shields.io/badge/Tupa-Strategy%20Runtime-0ea5e9" />
  <img alt="Rust" src="https://img.shields.io/badge/Rust-1.83-black?logo=rust" />
  <img alt="Docker Compose" src="https://img.shields.io/badge/Docker-Compose-2496ED?logo=docker&logoColor=white" />
  <img alt="Modes" src="https://img.shields.io/badge/Modes-paper%20%7C%20testnet%20%7C%20mainnet-0f766e" />
</p>
<!-- markdownlint-enable MD033 -->

Production-oriented Lead Trader runtime for Bybit Copy Trading, built with Rust microservices and powered by TupaLang.

ViperTrade is designed to run a deterministic trading decision pipeline with strong
operational controls: health checks, replayable validation, paper/testnet/mainnet
execution modes, reconciliation, audit-friendly events, operator controls, and a web
dashboard for live visibility.

## Why this project exists

Most trading bots fail in production for reasons that go beyond signal quality:

- strategy logic becomes hard to audit
- runtime behavior drifts from the original design
- exchange-side edge cases break assumptions
- operators lack safe controls during incidents
- validation and release discipline is too weak for real capital

ViperTrade exists to close that gap between strategy design and production execution.

## Why TupaLang matters here

TupaLang gives this project a clear strategy layer instead of forcing all decision logic into application code.

In ViperTrade, Tupa helps by:

- separating strategy intent from runtime plumbing
- validating the trading pipeline before runtime
- allowing the strategy service to load a validated plan at startup
- reducing hot-path complexity inside the Rust strategy service
- making strategy changes easier to review, audit, and reason about
- supporting a safer production workflow where pipeline changes can be validated before deployment

Tupa is not used here as a demo integration. It is part of the production architecture of the system.

## Production mindset

This repository is organized around repeatability and operational safety:

- deterministic Rust services
- Docker-based local/runtime parity
- health-first operational workflows
- staged execution modes: `paper`, `testnet`, `mainnet`
- API-based operator controls
- kill switch and reconciliation workflows
- CI validation before commit/push
- evidence-oriented release documentation

## Architecture

Core services:

- `market-data`
  - ingests and normalizes exchange signals
- `strategy`
  - loads the Tupa-derived runtime plan and produces decisions
- `executor`
  - translates decisions into exchange-side actions
- `monitor`
  - checks reconciliation, drift, and runtime health
- `api`
  - exposes status, positions, trades, performance, and control endpoints
- `web`
  - provides the operator dashboard
- `postgres` and `redis`
  - persistence and event transport

## Runtime modes

- `paper`
  - live market prices with wallet and positions simulated in the database
- `testnet`
  - real exchange interaction on Bybit testnet
- `mainnet`
  - real exchange execution on Bybit mainnet

This lets the same system progress from simulation to controlled live operation without changing the operational model.

## Recommended environment

- WSL Fedora
- Docker Desktop on Windows with WSL integration enabled for Fedora
- Git

The automation in this repository assumes `docker compose` running inside WSL through Docker Desktop.
The supported compose entrypoint is `compose/docker-compose.yml` through `make compose-*`.

## Quick start

```bash
git clone https://github.com/marciopaiva/vipertrade.git
cd vipertrade
cp compose/.env.example compose/.env
make build-base-images
./scripts/init-secrets.sh
./scripts/security-check.sh
make compose-up
make health
```

## Daily operator workflow

Start the stack:

```bash
make compose-up
```

Check health:

```bash
make health
```

Validate the runtime end to end:

```bash
make validate-runtime
```

Run local CI parity before commit/push:

```bash
make validate-ci
```

Reset paper-trading data:

```bash
make data-reset-paper-db
```

Stop the stack:

```bash
make compose-down
```

## Make targets

The repository uses `make` as the main operator and developer interface.

Main commands:

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

## Validation model

Fast local Rust checks:

```bash
make validate-workspace-quick
```

Full local validation:

```bash
make validate-full
```

Pre-push validation aligned with GitHub Actions:

```bash
make validate-ci
```

Strict docs lint on top of local CI:

```bash
CI_LOCAL_STRICT_DOCS=1 ./scripts/ci-local.sh
```

## Builder-based Rust validation

After building the base images, you can run Rust checks inside the standard builder image without depending on the host toolchain:

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

Current documentation is organized by intent:

- `docs/spec/`
  - modular technical specification
- `docs/operations/`
  - live operational procedures, gates, and policies
- `docs/operations/evidence/`
  - dated validation and release evidence
- `docs/releases/`
  - release-facing documentation
- `docs/legacy/`
  - historical source material

Recommended entry points:

- `docs/README.md`
- `docs/spec/README.md`
- `docs/spec/07-configuration.md`
- `docs/operations/RUNBOOK.md`
- `docs/releases/RELEASE_CHECKLIST.md`

## CI

GitHub Actions runs on push and pull request:

- Rust: `cargo fmt --check` + `cargo check --workspace --locked`
- Web: `yarn install --frozen-lockfile` + `yarn build`

Workflow:

- `.github/workflows/ci.yml`
- `.github/workflows/ci-local-parity.yml`

## Operational note

This project is intended for disciplined operational use.

That means:

- staged rollout through `paper` and `testnet`
- explicit runtime controls
- evidence-based release decisions
- strong handling of exchange credentials and risk settings

Use `mainnet` only when the surrounding operational process is ready for it.

## Risk disclosure

ViperTrade does not guarantee profits, capital preservation, or any specific trading outcome.

This software is an execution and decision-support system. Real results depend on
market conditions, exchange behavior, latency, liquidity, slippage, strategy
configuration, and operator decisions.

Safe use requires deliberate configuration, including:

- token universe selection
- per-token threshold tuning
- risk profile selection
- position sizing
- entry and exit thresholds
- execution mode selection (`paper`, `testnet`, `mainnet`)
- operational monitoring and rollback readiness

Different token sets and threshold combinations can materially change runtime behavior and risk.

Use this software entirely at your own risk. The operator is solely responsible for
strategy configuration, exchange credentials, capital allocation, and production
rollout decisions.
