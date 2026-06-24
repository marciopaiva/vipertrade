# 05 - Runtime and Operations

## Error Handling and Resilience

- Error domains: Bybit REST, cross-exchange REST (Binance/OKX), database, Redis, and the
  risk/decision engine.
- Retry with bounded backoff for transient REST failures.
- Critical failures pause new entries and surface through health/observability.
- Market-data is **REST-poll based** (no exchange WebSocket): a failed fetch for one
  symbol skips that symbol for the cycle and the rest still publish; a failed BTC macro
  refresh skips the whole cycle and retries on the next tick.

## Ingestion Resilience (REST polling)

- `market-data` runs a fixed-cadence cycle (~5s); each tick re-fetches klines for every
  enabled symbol across bybit ∩ binance ∩ okx and recomputes signals.
- Stale/incomplete cross-exchange data drops the affected symbol rather than emitting a
  half-formed signal (signal `validate()` gate before publish).
- Reconciliation against exchange truth is periodic and REST-based (see `monitor`).

## Disaster Recovery

- Incident classification: critical, high, medium, low.
- `kill_switch` (persisted in Postgres, read by the executor) contains losses immediately.
- Database restore followed by reconciliation against the exchange.
- API key revocation when compromise is suspected.
- Post-mortem for critical and high incidents.

## Secrets and Security Operations

- Secrets in `compose/.env` and `secrets/` with restricted permissions; never committed
  (enforced by `.gitignore` + `./scripts/security-check.sh`).
- Mode-scoped Bybit credentials; key rotation on a regular cadence with testnet validation.
- Pre-mainnet checklist: minimum API key permissions, 2FA, IP allowlists, no secrets in Git.

## Notifications and Monitoring

- Alert levels: `critical`, `warning`, `info`.
- Main alert types: circuit breaker, stop loss, trailing stop, daily summary.
- Operator-facing alerts; follower copy events remain controlled by Bybit.

## Tupa Integration Model

- The `ViperSmartCopy` strategy pipeline is **compiled into the strategy binary** via the
  `pipeline! { name: ViperSmartCopy, … }` macro in `services/strategy/src/lib.rs`
  (`tupa_core` + `tupa_engine::Executor`, runtime mode `in_process_tupa`). There is **no
  `.tp` file loaded at runtime** and no `TUPA_PIPELINE_PATH`.
- The macro defines the validated pipeline shape and structured step contracts; Rust steps
  reproduce the trading semantics and enrich step outputs with scores, reasons, and
  breakdowns.
- A canonical, human-readable spec of the pipeline is kept for reference only at
  `docs/spec/viper_smart_copy.reference.tp` (documentation, not loaded by the runtime).

## Trading Operations and Validation Modes

- Operates as Lead Trader in Bybit Copy Trading Classic with Smart Copy sizing constraints.
- Validation before production: deterministic backtest sweep (`/sweep`) + paper trading
  with real prices and simulated execution.

## Dynamic Trailing Stop

- Activation by minimum profit and progressive ratcheting; the trail only tightens.
- Driven by strategy runtime state; mirrored to Bybit native trailing on testnet/mainnet.
