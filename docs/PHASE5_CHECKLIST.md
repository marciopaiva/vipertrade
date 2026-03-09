# Phase 5 Checklist and Evidence

Date: 2026-03-08
Scope: Web observability and operator controls (testnet-safe)

## Checklist

- [x] Risk KPI panel available in web dashboard (`/api/v1/risk/kpis`).
- [x] Timeline panel available with events/signals/executions (`/api/v1/events`).
- [x] Service realtime status panel available (health checks for api, market-data, strategy, executor, monitor, backtest).
- [x] Secure web controls available (token-protected):
  - [x] Kill switch toggle (`POST /api/v1/control/kill-switch`)
  - [x] Executor pause/resume (`POST /api/v1/control/executor`)
  - [x] Risk limits update (`POST /api/v1/control/risk-limits`)
- [x] Audit persistence for operator actions in `system_events`.
- [x] Dashboard phase checklist + live evidence block rendered on UI.

## Evidence Commands

```bash
curl -s http://localhost:8080/api/v1/status
curl -s http://localhost:8080/api/v1/risk/kpis
curl -s 'http://localhost:8080/api/v1/events?limit=10'
curl -s http://localhost:3000/api/dashboard
```

## Expected Signals

- `status.db_connected=true`
- `status.operator_controls_enabled=true`
- `risk_kpis` object populated
- `events.items` non-empty during activity
- `/api/dashboard` returns `services[]` and `control_state`

## Audit Event Types (operator actions)

- `api_kill_switch_set`
- `api_executor_state_set`
- `api_risk_limits_set`
