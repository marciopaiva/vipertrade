# Reconciliation Evidence Bundle

## Objective

Provide repeatable operator evidence that monitor reconciliation is working with Bybit source-of-truth, persistence, and alert routing.

## Prerequisites

- Stack running (`./scripts/compose.sh up -d`)
- Monitor healthy (`./scripts/health-check.sh`)
- Environment configured in `compose/.env`:
  - `MAX_POSITION_DRIFT_NOTIONAL_USDT`
  - `ALERT_COOLDOWN_SEC`
  - Discord webhooks (`DISCORD_WEBHOOK_WARNING`, `DISCORD_WEBHOOK_CRITICAL`)

## 1) Service-Level Evidence

```bash
./scripts/compose.sh logs --since=20m monitor | grep -E "reconciliation: symbol=|alert suppressed by cooldown|bybit live query failed"
```

Expected:

- At least one `reconciliation: symbol=...` line per cycle and symbol.
- Optional `alert suppressed by cooldown` for repeated warning/error bursts.
- Optional Bybit fallback logs only when API is unavailable.

## 2) Database Evidence

```bash
podman exec -i vipertrade-postgres psql -U viper -d vipertrade <<'SQL'
\x on
SELECT symbol, reconciled, divergence, divergence_pct, snapshot_at
FROM position_snapshots
ORDER BY snapshot_at DESC
LIMIT 20;

SELECT event_type, severity, symbol, timestamp,
       data->>'drift_notional_usdt' AS drift_notional_usdt,
       data->>'drift_pct' AS drift_pct
FROM system_events
WHERE event_type = 'reconciliation_cycle'
ORDER BY timestamp DESC
LIMIT 20;
SQL
```

Expected:

- Fresh rows in `position_snapshots` for monitored symbols.
- `system_events` with matching severity and drift values.

## 3) Redis Event Evidence

```bash
podman exec -it vipertrade-redis redis-cli SUBSCRIBE viper:reconciliation
```

Expected payload fields:

- `event_type=reconciliation`
- `symbol`
- `severity`
- `drift_notional_usdt`
- `drift_pct`
- `reconciled`

## 4) Alert Policy Evidence

- `info`: no Discord message (noise control)
- `warning`: uses `DISCORD_WEBHOOK_WARNING`
- `error` and `critical`: use `DISCORD_WEBHOOK_CRITICAL`
- Repeated alerts with same `symbol + severity` are throttled by `ALERT_COOLDOWN_SEC`

## 5) Incident Handling Reference

Use [RUNBOOK](./RUNBOOK.md) section `11) Reconciliation Incident Playbook` for operator response steps.
