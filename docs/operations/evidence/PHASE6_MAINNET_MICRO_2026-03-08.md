# Phase 6 Mainnet Micro Attempt - 2026-03-08

## Decision

- decision: HOLD
- status: failed

## Summary

- Requested mode: `BYBIT_ENV=mainnet`, `EXECUTOR_ENABLE_LIVE_ORDERS=true`, allowlist `DOGEUSDT`
- Executor started in mainnet and passed market/time sanity
- Wallet-balance sanity precheck failed before order submission
- No orders were submitted in this attempt
- Environment was rolled back to safe posture (`BYBIT_ENV=testnet`, `EXECUTOR_ENABLE_LIVE_ORDERS=false`)

## Failure Signal

- `Bybit sanity checks failed with live orders enabled: wallet-balance failed: error decoding response body: EOF while parsing a value at line 1 column 0`

## Artifact

- docs/operations/artifacts/phase6/phase6_mainnet_micro_20260308T215704Z.json
