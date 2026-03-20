# Phase 2 Validation Window - 2026-03-08

## Context

- Date: 2026-03-08
- Environment: WSL Fedora + Podman
- Scope: monitor reconciliation maturity validation (Bybit source-of-truth + persistence + alert policy)

## Controlled Window Setup

- Temporary setting for accelerated observation:
  - `RECONCILIATION_INTERVAL_MIN=1`
- After evidence capture, restored to baseline:
  - `RECONCILIATION_INTERVAL_MIN=5`

## Evidence Collected

### Monitor Runtime (container logs)

- `Monitor config: health_interval=900s reconciliation_interval=60s max_drift=5 USDT cooldown=300s bybit_env=testnet`
- `Connected to PostgreSQL for reconciliation`
- Reconciliation loop samples:
  - `reconciliation: symbol=DOGEUSDT local=0 bybit=0 drift=0 severity=info`
  - `reconciliation: symbol=XRPUSDT local=0 bybit=0 drift=0 severity=info`
  - `reconciliation: symbol=TRXUSDT local=0 bybit=0 drift=0 severity=info`
  - `reconciliation: symbol=XLMUSDT local=0 bybit=0 drift=0 severity=info`

### PostgreSQL Evidence

Event summary:

- `executor_event_processed|12401`
- `reconciliation_cycle|48`
- `executor_reconciliation|19`

Latest snapshots:

- `XLMUSDT|t|0|0|2026-03-08 15:28:04.966528+00`
- `TRXUSDT|t|0|0|2026-03-08 15:28:04.964327+00`
- `XRPUSDT|t|0|0|2026-03-08 15:28:04.961997+00`
- `DOGEUSDT|t|0|0|2026-03-08 15:28:04.959221+00`

## Bug Found and Fixed During Validation

- Symptom:
  - monitor logged `failed to resolve bybit notional ... no rows returned by a query that expected to return at least one row`
- Root cause:
  - snapshot fallback query used `fetch_one` and failed when table had no prior row for symbol.
- Fix applied:
  - changed fallback path to `fetch_optional` with default `0.0`.
- Result:
  - reconciliation continued normally and persisted rows for all monitored symbols.

## Exit-Criteria Check (Phase 2)

- Deterministic loop output per interval/symbol: **PASS**
- Drift persistence/events in DB: **PASS**
- Operator playbook and evidence procedure documented: **PASS**
- Alert policy noise controls (info suppressed + cooldown): **PASS**

## Baseline Restored

- `RECONCILIATION_INTERVAL_MIN=5`
- monitor restarted with baseline config.
