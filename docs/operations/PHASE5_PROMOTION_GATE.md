# Phase 5 Promotion Gate - Smart Copy and Trailing

## Objective

Define objective criteria to promote Phase 5 from baseline validation to optimization-complete status.

## Gate Stages

1. Baseline Validation (mandatory)
2. Smart Copy Band Stability (mandatory)
3. Trailing Profile Coverage (mandatory)

## Pass Criteria Matrix

| Stage | Required Evidence | Minimum Criteria | Decision |
|---|---|---|---|
| Baseline Validation | `PHASE5_BASELINE_<date>.md` + JSON artifact | `issues=0`, stack healthy, API/DB consistency pass | Go / Hold |
| Smart Copy Band Stability | baseline JSON signals | smart-copy-compatible in-band ratio >= 0.95 (window) | Go / Hold |
| Trailing Profile Coverage | baseline JSON signals | all enabled pairs have complete `by_profile` configs for all profiles | Go / Hold |

## Stop Conditions

- Smart Copy out-of-band ratio above threshold.
- Missing or malformed trailing profile config for any enabled pair.
- Runtime/API inconsistency in validation window.

## Required Evidence Bundle

- `docs/operations/evidence/PHASE5_BASELINE_<date>.md`
- `docs/operations/artifacts/phase5/phase5_baseline_<timestamp>.json`
- Relevant logs and incident notes (if any)
