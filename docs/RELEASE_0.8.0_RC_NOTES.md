# Release Notes - ViperTrade 0.8.0-rc

Date: 2026-03-08

## Scope

- Executor live flow hardening (Bybit testnet).
- Fill persistence and idempotency enforcement.
- Reconciliation corrective mode validation.
- CI expansion with executor DB integration.

## Implemented

- Added `bybit_fills` persistence path in executor and schema support.
- Enforced strong idempotency with unique index on `system_events` for `executor_event_processed` + `source_event_id`.
- Added optional `EXECUTOR_RECONCILE_FIX` mode for controlled local correction when local > Bybit.
- Fixed close-path DB constraint issue (`trades.quantity > 0`) by avoiding `quantity=0` writes on closed rows.
- Added CI job `Executor DB Integration` using ephemeral PostgreSQL and schema apply.
- Added explicit reconciliation telemetry split: `executor_reconciliation_detected` vs `executor_reconciliation_fix_applied`.

## Operational Evidence

- Live testnet cycle completed for `DOGEUSDT` (ENTER + CLOSE).
- Executor logs confirmed order submission and close reconciliation after fix.
- `bybit_fills` recorded fills during live validation.
- Reconciliation corrective window with `EXECUTOR_RECONCILE_FIX=true` reduced local open qty from 200 to 0.
- Manual test data cleanup executed (`trades`, `system_events`, `bybit_fills`).

## CI Status

- `CI` (main push) succeeded for commit `0925e50`.
- Earlier `CI Local Parity` failure was markdown lint (`README.md` extra blank line); fixed in this follow-up.

## Technical Debt / Follow-ups

- Keep `EXECUTOR_ENABLE_LIVE_ORDERS=false` by default; enable only for controlled windows.
- Continue monitoring reconciliation events for false positives under volatile conditions.
- Reconciliation telemetry now distinguishes detect vs fix-applied; monitor both event streams during rollout windows.

## Rollback Assets

- Up migration: `database/migrations/20260307_001_executor_fills_idempotency.sql`
- Down migration: `database/migrations/20260308_002_executor_fills_idempotency_down.sql`
