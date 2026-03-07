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

./scripts/compose.sh up -d --no-deps executor
./scripts/compose.sh logs -f executor
```

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
