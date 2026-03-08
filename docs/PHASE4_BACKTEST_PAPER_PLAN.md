# Phase 4 - Backtesting and Paper-to-Live Validation Plan

## Objective

Establish reproducible backtest and paper-trading validation gates before any promotion to controlled live windows.

## Baseline Inputs

- `VIPERTRADE_SPEC.md` sections 14 and 18.
- `docs/PROJECT_PHASES.md` (Phase 4 in progress).
- Completed evidence from Phase 1, 2, and 3.

## Workstreams

## 1) Backtest Reproducibility

Scope:

- Define deterministic backtest input windows and seed strategy configuration.
- Ensure repeated runs produce stable metrics deltas within explicit tolerance.

Deliverables:

- Backtest run command contract (inputs, outputs, metrics).
- Artifact retention format (JSON/CSV/report markdown).

## 2) Paper Trading Regression Gates

Scope:

- Validate end-to-end runtime in paper mode with strategy, executor, and monitor.
- Track decision-to-execution and reconciliation behavior over rolling windows.

Deliverables:

- Regression checklist for paper mode.
- Minimum pass criteria for risk and reconciliation stability.

## 3) Promotion Criteria to Controlled Live Window

Scope:

- Define objective thresholds and rollback criteria.

Deliverables:

- Promotion gate table with required evidence.
- Stop/go checklist aligned with release process.

## Execution Order

1. Baseline runtime and backtest readiness checks.
2. Reproducible backtest run and artifact capture.
3. Paper-mode regression window with reconciliation/risk checks.
4. Promotion gate decision package.

## Exit Criteria

- Backtest workflow is reproducible and evidence artifacts are versioned.
- Paper-mode regression passes without critical reconciliation drift.
- Promotion package is complete and reviewable without manual log digging.

## Current Progress

- Item 1 started: baseline validation script introduced (`scripts/phase4-validate.sh`).
- Initial baseline evidence generated in `docs/operations/PHASE4_BASELINE_2026-03-08.md`.
- Backtest run contract documented in `docs/operations/PHASE4_BACKTEST_CONTRACT.md`.
- Deterministic run script added: `scripts/phase4-backtest-run.sh`.
- Initial run evidence generated in `docs/operations/PHASE4_BACKTEST_RUN_2026-03-08.md`.
- JSON artifacts generated in `docs/operations/artifacts/backtest/`.