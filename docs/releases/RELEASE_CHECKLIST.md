# Release Checklist (WSL Fedora + Docker Desktop)

## Preflight

- Ensure clean workspace: `git status --short`
- Run local CI parity: `make validate-ci`
- Optional strict docs lint: `CI_LOCAL_STRICT_DOCS=1 ./scripts/ci-local.sh`
- Validate pipeline: `./scripts/validate-pipeline.sh`
- Runtime baseline: `make validate-full`

## Runtime Validation

- Start and validate the runtime:
  - `make compose-up`
  - `make validate-runtime`
- Confirm subscribers:
  - `docker exec vipertrade-redis redis-cli PUBSUB NUMSUB viper:market_data viper:decisions`
- Confirm strategy/executor activity:
  - `./scripts/compose.sh logs --tail 80 strategy`
  - `./scripts/compose.sh logs --tail 80 executor`

## Rollback Plan

- DB rollback migration available:
  - `database/migrations/20260308_002_executor_fills_idempotency_down.sql`

- Stop current stack:
  - `make compose-down`
- Revert to last known-good commit:
  - `git checkout <known-good-sha>`
- Reapply env and bring the stack back up:
  - `make compose-up`
  - `make health`

## Release Evidence

Final release evidence checkpoints (2026-03-08):

Phase 1 closure evidence:

- Controlled testnet smoke passed (`DOGEUSDT` ENTER/CLOSE) with executor statuses `submitted` + `submitted_close`.
- Fill persistence verified in `bybit_fills` during smoke window.
- Idempotency check passed (`duplicate source_event_id = 0`).
- Manual smoke data cleanup executed after validation.

Phase 3 closure evidence:

- Validation report published: `docs/operations/evidence/PHASE3_VALIDATION_2026-03-08.md`.
- `/api/v1` operational read endpoints validated in runtime and CI.
- Performance parity check script passing against DB aggregates.
- Kill-switch API validated with deny-by-default (`403`) and positive operator flow (`enable`/`disable`) with DB audit evidence.

- Capture command outputs for:
  - `make validate-ci`
  - `make validate-runtime`
- Link passing GitHub CI run for release commit.
