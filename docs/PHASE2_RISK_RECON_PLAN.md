# Phase 2 - Risk and Reconciliation Start

## Scope

Start the functional baseline for monitor-driven risk controls and reconciliation.

## Implemented in this step

- Added monitor runtime config via environment:
  - `HEALTH_CHECK_INTERVAL_SEC` (default: 60)
  - `RECONCILIATION_INTERVAL_SEC` (default: 300)
  - `MAX_POSITION_DRIFT_NOTIONAL_USDT` (default: 5.0)
- Added periodic heartbeat scheduler.
- Added periodic reconciliation scheduler placeholder with drift threshold logging.

## Next Iterations

1. Persist reconciliation snapshots in PostgreSQL.
2. Compare local positions vs Bybit positions per symbol.
3. Emit structured reconciliation events to Redis.
4. Add alert routing for drift violations.
