# Operations Runbook (WSL Fedora + Podman)

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
podman exec -i vipertrade-postgres psql -U viper -d vipertrade <<'SQL'
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
