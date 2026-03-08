# Phase 6 - Mainnet Micro and Production Readiness Plan

## Objective

Validate operational readiness for a controlled mainnet micro rollout with tested rollback and disaster-recovery routines.

## Baseline Inputs

- `docs/PROJECT_PHASES.md` (Phase 6 scope and exit criteria)
- `docs/operations/RUNBOOK.md` (operational procedures)
- Latest Phase 4/5 decision packages and artifacts

## Workstreams

## 1) Mainnet Micro Readiness Gates

Scope:

- Validate stack health, API/DB consistency, and secure runtime posture.
- Ensure live orders remain disabled outside controlled windows.

Deliverables:

- Automated readiness gate script.
- Baseline evidence artifact for repeatable execution.

## 2) Rollback and Operator Controls

Scope:

- Validate kill-switch enable/disable path with operator token.
- Measure rollback reaction time within a bounded threshold.

Deliverables:

- Rollback drill result included in readiness artifacts.

## 3) Disaster Recovery Drill (Schema Backup)

Scope:

- Validate schema backup command path and evidence capture.

Deliverables:

- Schema backup drill output hash/size in readiness artifact.

## Exit Criteria

- Readiness gate passes with `issues=0`.
- Rollback path is tested and within target time budget.
- DR backup drill evidence is available and reproducible.
