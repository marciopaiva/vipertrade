# API Access Matrix

## Public Endpoints

- `GET /api/v1/health`
- `GET /api/v1/status`
- `GET /api/v1/positions`
- `GET /api/v1/trades?limit=<n>`
- `GET /api/v1/performance`

Auth: none.

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
