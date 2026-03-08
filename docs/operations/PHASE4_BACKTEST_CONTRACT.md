# Phase 4 Backtest Run Contract

## Objective

Define a deterministic run interface and evidence artifact format for Phase 4.

## Command

```bash
./scripts/phase4-backtest-run.sh \
  --window-start 2026-02-01T00:00:00Z \
  --window-end   2026-03-01T00:00:00Z \
  --seed 42 \
  --profile MEDIUM
```

## Required Inputs

- `window_start` (UTC ISO-8601)
- `window_end` (UTC ISO-8601)
- `seed` (integer)
- `profile` (`CONSERVATIVE|MEDIUM|AGGRESSIVE`)

## Determinism Rules

- Stable run key derived from `(window_start, window_end, seed, profile, git_sha)`.
- Input file hashes captured in artifact:
  - `config/strategies/viper_smart_copy.tp`
  - `config/trading/pairs.yaml`
  - `config/system/profiles.yaml`
- Run must fail if required input files are missing.

## Artifact Outputs

- JSON artifact path:
  - `docs/operations/artifacts/backtest/backtest_<timestamp>_seed<seed>.json`
- Markdown evidence snapshot path:
  - `docs/operations/PHASE4_BACKTEST_RUN_<YYYY-MM-DD>.md`

## JSON Schema (practical)

```json
{
  "schema_version": "v1",
  "run_id": "string",
  "created_at_utc": "string",
  "git_sha": "string",
  "window": { "start_utc": "string", "end_utc": "string" },
  "seed": 42,
  "profile": "MEDIUM",
  "input_hashes": {
    "pipeline_tp": "sha256",
    "pairs_yaml": "sha256",
    "profiles_yaml": "sha256"
  },
  "checks": {
    "backtest_health_http": 200,
    "service_available": true,
    "metrics_collected": true
  },
  "metrics": {
    "total_trades": 0,
    "win_rate": 0.0,
    "total_pnl": 0.0,
    "max_drawdown": 0.0
  },
  "status": "baseline_collected"
}
```

## Notes

- Metrics are collected from persisted DB data within the selected run window.
- If DB metrics cannot be collected, run status must be `baseline_partial`.
