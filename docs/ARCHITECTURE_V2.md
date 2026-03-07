# ViperTrade Architecture v2 (WSL Fedora + Podman)

## Goals
- Keep local/dev environment aligned with production behavior.
- Execute strategy logic deterministically using Tupa.
- Reduce runtime fragility by removing per-event CLI subprocess execution.
- Preserve full auditability of trade decisions.

## Runtime Topology
- market-data: ingests Bybit streams and normalizes market events.
- strategy: consumes normalized events and evaluates Tupa execution plan.
- executor: enforces idempotency and sends orders to Bybit.
- monitor: enforces global risk controls and health supervision.
- api/web: read/control plane only.
- postgres: source of truth for positions, decisions and audit trail.
- redis: event bus and transient state.

## Tupa Integration Model
### Control Plane (build/startup)
- Validate pipeline source with `tupa check`.
- Generate immutable plan artifact via `tupa codegen --plan-only`.
- Persist pipeline_version, plan_hash, and metadata in DB.

### Data Plane (hot path)
- Load plan once on strategy startup.
- Execute decisions with tupa-runtime (embedded), not `tupa run` per event.
- Persist decision_hash, execution_hash, constraints_satisfied per decision.

## Event Contracts
- Input stream: viper:market_data (normalized schema only).
- Decision stream: viper:decisions.
- Every message must include: event_id, symbol, timestamp, schema_version.

## Podman Standards
- Use `./scripts/compose.sh` as default compose entrypoint (bridge mode).
- Keep `./scripts/compose-host.sh` only as local WSL fallback.
- Prefer bridge network over `network_mode: host` for service isolation.
- Keep volumes explicit for: postgres, redis, audit logs, plan cache.
- Healthchecks must verify dependencies (DB/Redis/Bybit reachability).

## Security and Audit
- Secrets only from `.env` and `secrets/` mount.
- No secret material committed.
- Keep immutable audit records for:
  - input hash
  - output hash
  - decision hash
  - execution hash

## Non-Goals (Phase 1)
- Full strategy alpha tuning.
- Multi-exchange orchestration.
- Full production-grade SRE stack.