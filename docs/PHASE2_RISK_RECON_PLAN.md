# Phase 2 - Risk and Reconciliation Maturity

## Scope

Close monitor-driven risk controls and reconciliation with clear operational evidence.

## Implemented (current state)

- Monitor runtime configuration by environment:
  - `HEALTH_CHECK_INTERVAL_SEC` with fallback `HEALTH_CHECK_INTERVAL_MIN`
  - `RECONCILIATION_INTERVAL_SEC` with fallback `RECONCILIATION_INTERVAL_MIN`
  - `MAX_POSITION_DRIFT_NOTIONAL_USDT` (default: 5.0)
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

## Remaining Gaps to close Phase 2

1. Bybit source-of-truth integration in monitor:
   - fetch current position directly from Bybit API (not only latest stored snapshot)
   - compare against local open exposure per symbol in each reconciliation cycle
2. Alert policy hardening:
   - add deduplication/cooldown window for repeated symbol alerts
   - document escalation path and operator action matrix per severity
3. Operational evidence bundle:
   - SQL queries + log snippets proving drift detect/resolve behavior
   - runbook section for reconciliation incident handling

## Exit Criteria (Phase 2)

- Reconciliation loop produces deterministic records per interval and symbol.
- Drift threshold violations generate actionable low-noise alerts.
- Operators can run documented checks and recover from drift scenarios.
- CI local parity green with monitor changes.
