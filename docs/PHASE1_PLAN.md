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
  - `podman compose -f compose/docker-compose.yml config` passes.
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

## Risks
- Tupa API changes between RC versions.
- Redis transport semantics (Pub/Sub vs Streams) in migration.
- Host networking assumptions currently embedded in services.

## Exit Criteria
- Strategy service running deterministic plan in-process.
- End-to-end decision path stable in local Podman stack.
- Audit fields recorded for each decision.