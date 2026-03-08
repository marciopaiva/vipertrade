# Phase 3 - Lead Trader Operations and API Surface

## Objective

Deliver an operator-grade API surface for Lead Trader operations, with explicit control endpoints, public metrics consistency, and safety controls.

## Baseline Inputs

- `VIPERTRADE_SPEC.md` sections 13 and 20.
- Phase 2 completed with reconciliation controls and operational evidence.

## Current Progress

- Item 1 delivered (initial API capability map implementation):
  - `GET /api/v1/health`
  - `GET /api/v1/status`
  - `GET /api/v1/positions`
  - `GET /api/v1/trades`
  - `GET /api/v1/performance`
  - `POST /api/v1/control/kill-switch`
- Kill-switch control is deny-by-default when `OPERATOR_API_TOKEN` is not configured.
- Item 2 delivered (operator auth and access split):
  - middleware-based token auth on control routes
  - public vs operator route separation documented in `docs/operations/API_ACCESS_MATRIX.md`
- Item 3 delivered (metrics consistency layer):
  - deterministic windows with shared reference timestamp in `/api/v1/performance`
  - 6-decimal normalization for `win_rate` and `total_pnl`
  - consistency check script: `scripts/check-api-metrics-consistency.sh`

## Workstreams

## 1) API Capability Map

Target endpoints (minimum):

- `GET /api/v1/health` (service + dependency status)
- `GET /api/v1/status` (runtime mode, profile, kill-switch, risk flags)
- `GET /api/v1/positions` (open exposure by symbol/side)
- `GET /api/v1/trades` (recent lifecycle events)
- `GET /api/v1/performance` (PnL/win-rate/drawdown windows)
- `POST /api/v1/control/kill-switch` (operator action)

Deliverables:

- Contract table with request/response fields and error codes.
- Mapping each endpoint to data source (Postgres/Redis/service state).

## 2) Operator Auth and Access

Scope:

- Define operator-only controls vs public read endpoints.
- Add auth middleware path for control endpoints.

Deliverables:

- Environment variables for operator auth mode.
- Deny-by-default for control actions when auth missing/invalid.

## 3) Metrics Consistency Layer

Scope:

- Normalize metrics windows and definitions shared by API and dashboard.
- Guarantee deterministic rounding and timestamp boundaries.

Deliverables:

- Performance aggregation queries with explicit windows (24h, 7d, 30d).
- Consistency check script comparing API output with DB aggregates.

## 4) Kill-Switch and Runtime Controls

Scope:

- Implement kill-switch write path and runtime readback.
- Expose explicit reason + timestamp + operator actor fields.

Deliverables:

- Safety state persisted with audit trail.
- Rollback command and verification query documented.

## 5) Validation and Evidence

Scope:

- Define validation scenario matrix for API read/control paths.
- Generate evidence bundle similar to Phase 2 closure.

Deliverables:

- `docs/operations/PHASE3_VALIDATION_YYYY-MM-DD.md`
- API smoke commands + expected outputs.

## Execution Order

1. API capability map and contract freeze.
2. Auth and control protection.
3. Metrics consistency implementation.
4. Kill-switch end-to-end.
5. Validation window + evidence + phase sign-off.

## Exit Criteria

- Operator can audit and control runtime via API without direct DB access.
- Control endpoints are protected and auditable.
- Public metrics remain consistent with DB aggregates.
- CI local parity green for all API changes.
