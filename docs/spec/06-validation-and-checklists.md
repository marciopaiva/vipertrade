# 06 - Validation and Checklists

Source: `docs/legacy/VIPERTRADE_SPEC.md` (sections 18-20).

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

- Bootstrap and security: `make build-base-images`, `./scripts/init-secrets.sh`, `./scripts/security-check.sh`
- Compose runtime: `make compose-up`, `make compose-ps`, `make compose-logs`, `make compose-down`
- Validation: `make health`, `make validate-runtime`, `make validate-full`, `make validate-ci`
- Data reset: `make data-reset-paper-db`
- API operations: system status, positions, trades, leader stats, and kill switch via HTTP endpoints
- Database: SQL access inside the PostgreSQL container for operational queries

## API Surface (Current Spec)

- Portfolio, positions, trades, and performance.
- System status and kill switch.
- Copy trading and leader profile endpoints.
- WebSocket events for portfolio, positions, trades, and alerts.

## Original Reference

- `docs/legacy/VIPERTRADE_SPEC.md`, approximately lines 1768-1980.
