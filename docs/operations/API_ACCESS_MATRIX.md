# API Access Matrix

## Public Endpoints

- `GET /api/v1/health`
- `GET /api/v1/status`
- `GET /api/v1/positions`
- `GET /api/v1/trades?limit=<n>`
- `GET /api/v1/performance`

Auth: none.

## Runtime Source Matrix

- `GET /api/v1/status`
  - all modes: runtime/service status from API service + DB-backed control state
  - includes:
    - `trading_mode`
    - internal `trading_profile`
    - UI-facing `trade_profile_label` (`SMOKE` or `STANDARD`)
- `GET /api/v1/positions`
  - `PAPER`: database
  - `TESTNET`: Bybit testnet `position/list`
  - `MAINNET`: Bybit mainnet `position/list`
  - trailing fields in response remain locally derived for API/web observability, even when Bybit native trailing is also configured
- `GET /api/v1/trades?limit=<n>`
  - `PAPER`: database
  - `TESTNET`: Bybit testnet closed-PnL history
  - `MAINNET`: Bybit mainnet closed-PnL history
- `GET /api/v1/performance`
  - `PAPER`: database aggregates
  - `TESTNET`: Bybit testnet closed-PnL aggregates
  - `MAINNET`: Bybit mainnet closed-PnL aggregates

## Operator Endpoints

- `POST /api/v1/control/kill-switch`

Required headers:

- `x-operator-token: <OPERATOR_API_TOKEN>`
- `x-operator-id: <operator-id>` (optional, default `operator`)

Auth mode rules:

- `OPERATOR_AUTH_MODE=token`: validate `x-operator-token` against `OPERATOR_API_TOKEN`.
- Any other mode: control endpoints denied (`403 auth_not_configured`).
- Missing `OPERATOR_API_TOKEN`: control endpoints denied (`403 auth_not_configured`).

## Error Model (control endpoints)

- `401 invalid_token`: missing/invalid operator token.
- `403 auth_not_configured`: operator auth mode/token not configured.
- `503 db_unavailable`: database not connected.
- `500 persist_failed`: failed to persist control event.

## Quick Smoke

```bash
curl -s http://localhost:8080/api/v1/status | jq

curl -s -X POST http://localhost:8080/api/v1/control/kill-switch \
  -H "content-type: application/json" \
  -H "x-operator-token: ${OPERATOR_API_TOKEN}" \
  -H "x-operator-id: local-ops" \
  -d '{"enabled":true,"reason":"ops_test"}' | jq
```

## Metrics Consistency Check

Use the deterministic consistency check script:

```bash
./scripts/check-api-metrics-consistency.sh
```

It compares `/api/v1/performance` aggregates against direct PostgreSQL queries using the exact window bounds returned by the API.
