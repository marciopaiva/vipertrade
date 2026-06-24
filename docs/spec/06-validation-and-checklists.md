# 06 - Validation and Checklists

## Pre-Deploy Validation

- Security: `security-check` passes, `compose/.env` uses restricted permissions,
  `.env` stays out of Git, API keys have minimum required privileges, and 2FA/IP
  allowlists are enabled.
- Database: schema is applied, tables and indexes exist, and backups are defined.
- Services: containers boot correctly, health checks pass, Redis Pub/Sub works, and WebSocket reconnect behavior is validated.
- Risk: sizing and limits are validated, stop loss and trailing stop work, and both the circuit breaker and daily-loss protection are tested.
- Notifications: webhook is configured and both critical and warning alerts are delivered.
- Testing: paper trading is stable, stress backtests are approved, and both kill switch and error handling are exercised.
- Smart Copy and Lead Trader: sizing is within the target range, leverage matches the selected profile, and leader account plus metrics are ready.

## Runbook Commands

- Bootstrap (Compose): `./scripts/build-base-images.sh`, `./scripts/init-secrets.sh`, `./scripts/security-check.sh`
- Bootstrap (Kind): `./scripts/kind/prepare-wsl.sh`, `make build`
- Compose runtime: `./scripts/compose.sh up -d`, `./scripts/compose.sh ps`, `./scripts/compose.sh logs -f`, `./scripts/compose.sh down`
- Kind runtime: `make deploy`, `./scripts/kind/status.sh`, `./scripts/kind/delete.sh`
- Validation (health/runtime): `./scripts/health-check.sh all`, `./scripts/validate-runtime.sh bridge all`
- Validation (workspace): `./scripts/validate-workspace.sh all`, `./scripts/validate-workspace.sh ci`
- Data reset: `./scripts/reset-paper-db.sh --yes`

## API Surface (Current Spec)

- Portfolio, positions, trades, and performance.
- System status and kill switch.
- Copy trading and leader profile endpoints.
- WebSocket events for portfolio, positions, trades, and alerts.
