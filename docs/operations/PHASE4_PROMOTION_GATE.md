# Phase 4 Promotion Gate - Paper to Controlled Live

## Objective

Define objective criteria and rollback rules before promoting from paper mode to controlled live windows.

## Gate Stages

1. Paper Regression Gate (mandatory)
2. Testnet Micro Gate (mandatory)
3. Controlled Live Window Gate (time-boxed and reversible)

## Pass Criteria Matrix

| Stage | Required Evidence | Minimum Criteria | Decision |
|---|---|---|---|
| Paper Regression | `PHASE4_PAPER_REGRESSION_<date>.md` + JSON artifact | `issues=0`, health check pass, API/DB performance consistency pass, Redis subscribers present | Go / Hold |
| Testnet Micro | runtime logs + reconciliation snapshots + risk events | no critical reconciliation drift, no unresolved close failures, kill-switch functional | Go / Hold |
| Controlled Live | 24h-72h monitored window evidence | drawdown under profile cap, no critical alert storms, rollback path tested | Go / Rollback |

## Stop Conditions (Immediate Hold/Rollback)

- Critical risk breach (`critical` alerts repeated without mitigation).
- Reconciliation drift unresolved after automated and manual fix windows.
- Executor close path fails repeatedly for active positions.
- Metrics inconsistency between API and DB for core windows.

## Required Evidence Bundle

- `docs/operations/evidence/PHASE4_BACKTEST_RUN_<date>.md`
- `docs/operations/evidence/PHASE4_PAPER_REGRESSION_<date>.md`
- `docs/operations/artifacts/backtest/*.json`
- `docs/operations/artifacts/paper/*.json`
- Relevant runbook notes and incident entries (if any)

## Decision Record Template

```text
Date (UTC):
Stage:
Decision: GO | HOLD | ROLLBACK
Approver:
Evidence Links:
Notes:
```
