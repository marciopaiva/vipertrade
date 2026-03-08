# Phase 2 - Risk and Reconciliation Maturity

## Scope

Close monitor-driven risk controls and reconciliation with clear operational evidence.

## Implemented (current state)

- Monitor runtime configuration by environment:
  - `HEALTH_CHECK_INTERVAL_SEC` with fallback `HEALTH_CHECK_INTERVAL_MIN`
  - `RECONCILIATION_INTERVAL_SEC` with fallback `RECONCILIATION_INTERVAL_MIN`
  - `MAX_POSITION_DRIFT_NOTIONAL_USDT` (default: 5.0)
  - `ALERT_COOLDOWN_SEC` (default: 300)
- Periodic monitor heartbeat loop.
- Periodic reconciliation loop for `DOGEUSDT`, `XRPUSDT`, `TRXUSDT`, `XLMUSDT`.
- Drift computation and severity classification (`info`, `warning`, `error`, `critical`).
- Reconciliation persistence in PostgreSQL:
  - snapshots in `position_snapshots`
  - structured events in `system_events` (`event_type=reconciliation_cycle`)
- Structured reconciliation publish to Redis channel `viper:reconciliation`.
- Discord alert routing by severity:
  - `warning` -> `DISCORD_WEBHOOK_WARNING`
  - `error` and `critical` -> `DISCORD_WEBHOOK_CRITICAL`
  - `info` -> no alert (noise control)
- Alert dedup/throttling by `symbol + severity` using `ALERT_COOLDOWN_SEC`.
- Bybit source-of-truth pull in monitor:
  - authenticated GET `/v5/position/list` per symbol
  - notional from live position data, with snapshot fallback if API is unavailable
- Operational evidence and playbook:
  - [docs/operations/RECONCILIATION_EVIDENCE.md](./operations/RECONCILIATION_EVIDENCE.md)
  - runbook section `11) Reconciliation Incident Playbook`

## Remaining Gaps to close Phase 2

1. Execute a controlled validation window and attach evidence bundle to the phase closure note.
2. Confirm alert noise level in live-like volatility and tune `ALERT_COOLDOWN_SEC` if needed.

## Exit Criteria (Phase 2)

- Reconciliation loop produces deterministic records per interval and symbol.
- Drift threshold violations generate actionable low-noise alerts.
- Operators can run documented checks and recover from drift scenarios.
- CI local parity green with monitor changes.
