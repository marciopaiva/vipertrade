# Phase 5 - Smart Copy and Dynamic Trailing Optimization Plan

## Objective

Validate and optimize Smart Copy sizing stability and trailing-stop ratcheting behavior before mainnet micro rollout.

## Baseline Inputs

- `docs/PROJECT_PHASES.md` (Phase 5 scope and exit criteria)
- `config/trading/pairs.yaml` (pair risk + smart copy constraints)
- `config/system/profiles.yaml` (profile-specific trailing configs)
- Latest runtime evidence from Phase 4 controlled-live gate

## Workstreams

## 1) Smart Copy Sizing Stability

Scope:

- Ensure smart-copy-compatible trades remain inside configured notional band.
- Monitor deviations and classify out-of-band events.

Deliverables:

- Automated gate for in-band notional ratio.
- Evidence artifact with in-band/out-of-band summary.

## 2) Trailing Ratchet Coverage and Consistency

Scope:

- Verify every enabled pair has trailing stop profiles for all active risk profiles.
- Validate required trailing keys and ratchet-level structure.

Deliverables:

- Structural validation gate for trailing configs.
- Evidence artifact with per-pair coverage status.

## 3) Phase 5 Gate Package

Scope:

- Consolidate baseline checks and produce repeatable evidence.

Deliverables:

- `scripts/phase5-validate.sh`
- `docs/operations/PHASE5_BASELINE_<date>.md`
- `docs/operations/artifacts/phase5/phase5_baseline_<timestamp>.json`

## Exit Criteria

- Smart Copy in-band ratio meets threshold in observation window.
- Trailing profile coverage is complete for all enabled pairs.
- Phase 5 evidence package is reproducible and auditable.
