# Phase 1 Plan (Execution Foundation)

## Scope

Build a reliable execution backbone for WSL Fedora + Podman with deterministic Tupa strategy execution.

## Workstreams

### 1. Strategy Runtime Refactor

- Replace per-event `tupa run` subprocess in `services/strategy/src/main.rs`.
- Load Tupa plan at startup and keep runtime in memory.
- Standardize decision payload schema and publish to Redis.
- Definition of done:
  - Strategy service starts with plan validation.
  - No subprocess execution in hot loop.

### 2. Event and Domain Contracts

- Add shared domain module/crate for event payloads.
- Version payloads with schema_version.
- Definition of done:
  - market-data -> strategy -> executor use shared schema types.

### 3. Compose Hardening (Podman)

- Remove `network_mode: host` where possible.
- Add explicit network and health/readiness checks.
- Definition of done:
  - `./scripts/compose.sh config` passes.
  - Services expose deterministic health endpoints.

### 4. Local CI Parity

- Add local CI runner script to mirror core CI checks.
- Include Rust checks, Tupa pipeline validation and compose validation.
- Definition of done:
  - Single command validates local environment before commit.

## Deliverables

- docs/ARCHITECTURE_V2.md
- docs/PHASE1_PLAN.md
- scripts/ci-local.sh
- scripts/validate-runtime.sh

## Risks

- Tupa API changes between RC versions.
- Redis transport semantics (Pub/Sub vs Streams) in migration.
- Host networking assumptions currently embedded in services.

## Exit Criteria

- Strategy service running deterministic plan in-process.
- End-to-end decision path stable in local Podman stack.
- Audit fields recorded for each decision.

## Completion Status

Status: Completed on 2026-03-08.

### Exit Criteria Check

- Strategy service running deterministic plan in-process: Completed.
- End-to-end decision path stable in local Podman stack: Completed.
- Audit fields recorded for each decision: Completed.

### Final Evidence (2026-03-08)

- Baseline health: all core services healthy (`postgres`, `redis`, `market-data`, `strategy`, `executor`, `api`, `web`).
- Controlled testnet smoke (DOGEUSDT): `ENTER_LONG` + `CLOSE_LONG` successful.
- Executor statuses: `submitted` and `submitted_close` recorded with Bybit order ids.
- Fill persistence: `bybit_fills` populated during smoke run.
- Idempotency: duplicate `source_event_id` count remained 0.
- Post-validation hygiene: manual test data cleaned from `trades`, `system_events`, and `bybit_fills`.
- Safety reset: `EXECUTOR_ENABLE_LIVE_ORDERS=false` and `EXECUTOR_RECONCILE_FIX=false` restored.
