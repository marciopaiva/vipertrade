# Operations Runbook (WSL Fedora + Docker Desktop)

## 1) Bootstrap

```bash
cp compose/.env.example compose/.env
./scripts/init-secrets.sh
./scripts/security-check.sh
```

## 2) Start Stack

```bash
./scripts/compose.sh up -d
./scripts/health-check.sh
```

Fallback host mode:

```bash
./scripts/compose-host.sh up -d
./scripts/health-check.sh
```

## 3) Validate Runtime

```bash
./scripts/validate-runtime.sh bridge
```

Fallback host:

```bash
./scripts/validate-runtime.sh host
```

## 4) Logs and Diagnostics

```bash
./scripts/compose.sh logs -f strategy
./scripts/compose.sh logs -f executor
./scripts/compose.sh logs -f monitor
```

## 5) Stop Stack

```bash
./scripts/compose.sh down
```

Fallback host:

```bash
./scripts/compose-host.sh down
```

## 6) Full Local Validation (release gate)

```bash
./scripts/validate-workspace.sh
```

This generates a single report file under `logs/`.

## 7) Live Testnet Rollout (Gradual)

Default is safe dry-run (`EXECUTOR_ENABLE_LIVE_ORDERS=false`).

1. Keep live disabled and validate sanity checks/logs.
2. Set allowlist to one symbol only (example: `DOGEUSDT`).
3. Enable live orders and restart only executor.

```bash
# compose/.env
EXECUTOR_ENABLE_LIVE_ORDERS=true
EXECUTOR_LIVE_SYMBOL_ALLOWLIST=DOGEUSDT
BYBIT_ACCOUNT_TYPE=UNIFIED
EXECUTOR_RECONCILE_FIX=false

./scripts/compose.sh up -d --no-deps executor
./scripts/compose.sh logs -f executor
```

Reconciliation behavior:

- `EXECUTOR_RECONCILE_FIX=false` (default): detect/log only.
- `EXECUTOR_RECONCILE_FIX=true`: applies conservative local reductions when local qty > Bybit qty.

Smoke publish for ENTER order path:

```bash
./scripts/publish-test-decision.sh DOGEUSDT ENTER_LONG 10
```

## 8) Rollback (Immediate)

```bash
# compose/.env
EXECUTOR_ENABLE_LIVE_ORDERS=false

./scripts/compose.sh up -d --no-deps executor
./scripts/compose.sh logs -f executor
```


## 9) DB Rollback (fills/idempotency patch)

If needed, rollback only the latest executor DB patch:

```bash
docker exec -i vipertrade-postgres psql -U viper -d vipertrade <<'SQL'
DROP INDEX IF EXISTS uq_system_events_executor_source_event;
DROP TABLE IF EXISTS bybit_fills;
SQL
```

Then restart executor in safe mode:

```bash
# compose/.env
EXECUTOR_ENABLE_LIVE_ORDERS=false
EXECUTOR_RECONCILE_FIX=false

./scripts/compose.sh up -d --no-deps executor
```

## 10) Go/No-Go Checklist (live testnet)

Go:

- `health-check.sh` without database errors
- executor logs show `Submitted Bybit order` and no `submitted_close_no_persist`
- `bybit_fills` has rows for the smoke cycle
- no duplicate `source_event_id` in `executor_event_processed`

No-Go:

- Bybit errors like `110017`/`110094` in smoke cycle
- any DB constraint error during close reconciliation
- reconciliation diff persists after controlled `EXECUTOR_RECONCILE_FIX=true` window

## 11) Reconciliation Incident Playbook

1. Confirm monitor health and recent cycles:

```bash
./scripts/health-check.sh
./scripts/compose.sh logs --since=20m monitor | grep -E "reconciliation: symbol=|bybit live query failed|alert suppressed by cooldown"
```

2. Check divergence and severity in database:

```bash
docker exec -i vipertrade-postgres psql -U viper -d vipertrade <<'SQL'
SELECT symbol, reconciled, divergence, divergence_pct, snapshot_at
FROM position_snapshots
ORDER BY snapshot_at DESC
LIMIT 20;

SELECT severity, symbol, timestamp, data->>'drift_notional_usdt' AS drift_notional_usdt
FROM system_events
WHERE event_type = 'reconciliation_cycle'
ORDER BY timestamp DESC
LIMIT 20;
SQL
```

3. Operator action matrix:

- `warning`: monitor for 2-3 cycles; no emergency action.
- `error`: pause new entries if persistent; validate Bybit/account connectivity.
- `critical`: pause entries immediately and investigate position mismatch before resuming.

4. Validate alert throttling:

- Ensure repeated same `symbol+severity` events are not flooding Discord.
- Adjust `ALERT_COOLDOWN_SEC` in `compose/.env` if needed, then restart monitor:

```bash
./scripts/compose.sh up -d --no-deps monitor
```

5. Capture evidence bundle:

- Follow [RECONCILIATION_EVIDENCE](./RECONCILIATION_EVIDENCE.md)
- Attach logs + SQL outputs to release/incident notes.

## 12) API Kill-Switch Playbook

Pre-requisites:

- `OPERATOR_API_TOKEN` configured in `compose/.env`
- API service running and healthy

Quick status:

```bash
./scripts/kill-switch-control.sh status
```

Enable kill-switch (halt mode):

```bash
OPERATOR_API_TOKEN="${OPERATOR_API_TOKEN}" \
REASON="incident_reconciliation" \
./scripts/kill-switch-control.sh enable
```

Disable kill-switch (resume):

```bash
OPERATOR_API_TOKEN="${OPERATOR_API_TOKEN}" \
REASON="incident_resolved" \
./scripts/kill-switch-control.sh disable
```

Manual API smoke (optional):

```bash
curl -s -X POST http://localhost:8080/api/v1/control/kill-switch \
  -H "content-type: application/json" \
  -H "x-operator-token: ${OPERATOR_API_TOKEN}" \
  -H "x-operator-id: local-ops" \
  -d '{"enabled":true,"reason":"ops_test"}' | jq
```

Rollback verification checklist:

- API returns `kill_switch.enabled=false` after disable
- Latest `system_events` row with `event_type=api_kill_switch_set` has expected actor/reason
- `risk_status` in `/api/v1/status` transitions according to kill-switch state

SQL verification query:

```bash
docker exec -i vipertrade-postgres psql -U viper -d vipertrade -At -F '|' -c \
"SELECT event_type,severity,data->>'enabled',data->>'reason',data->>'actor',to_char(timestamp,'YYYY-MM-DD HH24:MI:SS')
 FROM system_events
 WHERE event_type='api_kill_switch_set'
 ORDER BY timestamp DESC
 LIMIT 5;"
```

## 13) Phase 5 Baseline (Smart Copy + Trailing)

Run the phase gate and generate baseline evidence:

```bash
./scripts/phase5-validate.sh
```

Optional tuning knobs:

```bash
WINDOW_HOURS=48 SMART_COPY_MIN_IN_BAND_RATIO=0.97 ./scripts/phase5-validate.sh
```

## 14) Phase 6 Baseline (Mainnet Readiness)

Run readiness baseline and generate evidence:

```bash
./scripts/phase6-validate.sh
```

Optional rollback SLO tuning:

```bash
MAX_ROLLBACK_SEC=20 ./scripts/phase6-validate.sh
```

## 15) Phase 6 Testnet Micro Gate (No-Mainnet Policy)

Run the operational gate without mainnet order submission:

```bash
./scripts/phase6-testnet-micro-gate.sh
```

Policy reference:

- `docs/operations/PHASE6_NO_MAINNET_POLICY.md`
