# Phase 6 Promotion Gate - Testnet/Simulation Readiness

## Objective

Define objective criteria for promoting from readiness baseline to production-readiness under no-mainnet policy.

## Gate Stages

1. Readiness Baseline (mandatory)
2. Testnet Micro Gate (mandatory)
3. Rollback Drill (mandatory)
4. DR Backup Drill (mandatory)

## Pass Criteria Matrix

| Stage | Required Evidence | Minimum Criteria | Decision |
|---|---|---|---|
| Readiness Baseline | `PHASE6_BASELINE_<date>.md` + JSON artifact | `issues=0`, health and API/DB consistency pass, security check pass, live disabled by default | Go / Hold |
| Testnet Micro Gate | `PHASE6_TESTNET_MICRO_<date>.md` + JSON artifact | `issues=0`, testnet mode confirmed, rollback flow pass | Go / Hold |
| Rollback Drill | baseline/testnet signals | kill-switch enable/disable passed, rollback elapsed <= threshold | Go / Hold |
| DR Backup Drill | baseline signals | schema backup drill succeeded and output is non-empty | Go / Hold |

## Stop Conditions

- Security check fails or critical secrets hygiene issue detected.
- Kill-switch drill fails.
- Runtime not in safe posture outside controlled window (`EXECUTOR_ENABLE_LIVE_ORDERS=true`).
- DR backup drill fails.
- Any order-submission test attempted on mainnet (policy violation).

## Required Evidence Bundle

- `docs/operations/PHASE6_BASELINE_<date>.md`
- `docs/operations/PHASE6_TESTNET_MICRO_<date>.md`
- `docs/operations/artifacts/phase6/phase6_baseline_<timestamp>.json`
- `docs/operations/artifacts/phase6/phase6_testnet_micro_<timestamp>.json`
- `docs/operations/PHASE6_DECISION_PACKAGE_<date>.md`
