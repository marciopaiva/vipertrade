# Phase 6 No-Mainnet Policy

## Policy

- No runtime validation that submits orders on Bybit mainnet.
- All Phase 6 operational validation must run on `testnet` and/or simulation paths.

## Scope

- Applies to readiness, rollback drills, and micro-window evidence during Phase 6.
- Mainnet execution remains explicitly out of scope for this project cycle.

## Operational Rules

- Keep `BYBIT_ENV=testnet` in `compose/.env`.
- Keep `EXECUTOR_ENABLE_LIVE_ORDERS=false` by default.
- If live-order checks are needed, run controlled micro cycles only on testnet and immediately revert to safe posture.

## Acceptance Mapping

- A Phase 6 decision can be `GO (testnet/sim readiness)` while preserving `HOLD (mainnet execution)`.
