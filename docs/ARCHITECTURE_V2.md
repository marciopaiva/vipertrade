# ViperTrade Architecture v2 (WSL Fedora + Docker Desktop)

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
- postgres: source of truth for decisions, audit trail, and paper-mode wallet/positions/trades.
- redis: event bus and transient state.

Runtime source-of-truth by mode:

- `PAPER`: public prices from Bybit mainnet, wallet/positions/trades/performance from database simulation.
- `TESTNET`: wallet/positions/trades/performance from Bybit testnet APIs.
- `MAINNET`: wallet/positions/trades/performance from Bybit mainnet APIs.

Operational trade-profile labels:

- `TESTNET / SMOKE`: permissive smoke overlay for buy/sell/reconcile validation.
- `PAPER / STANDARD`: guarded rules with simulated persistence.
- `MAINNET / STANDARD`: guarded rules with real exchange persistence/execution.

`TESTNET / SMOKE` keeps a mode-level overlay on top of pair config:

- relaxed entry requirements to increase smoke-cycle coverage
- `DOGEUSDT` size capped to `8 USDT`
- `stop_loss_pct = 3%`
- fixed take profit disabled
- trailing enabled at `+0.30%` with break-even at `+0.40%`
- tighter trailing ratchets every `+0.10%`
- `min_hold_seconds = 45`

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

## Container Runtime Standards

- Use `./scripts/compose.sh` as default compose entrypoint (bridge mode).
- Use Docker Desktop + WSL as the standard local runtime.
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

## Trailing Stop Model

- Strategy maintains local trailing state for observability and deterministic API/web snapshots.
- Executor also configures Bybit native trailing stop after successful `TESTNET`/`MAINNET` entries.
- Native trailing setup is retried with short backoff to absorb exchange timing races while the position becomes visible.
- Current runtime is intentionally hybrid:
  - local trailing remains the fallback control plane
  - exchange-native trailing gives direct protection and exchange-side visibility

## Non-Goals (Phase 1)

- Full strategy alpha tuning.
- Multi-exchange orchestration.
- Full production-grade SRE stack.
