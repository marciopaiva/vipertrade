# Release Checklist (WSL Fedora + Podman)

## Preflight

- Ensure clean workspace: `git status --short`
- Run local CI: `./scripts/ci-local.sh`
- Optional strict docs lint: `CI_LOCAL_STRICT_DOCS=1 ./scripts/ci-local.sh`
- Validate pipeline: `./scripts/validate-pipeline.sh`

## Runtime Validation (Bridge Primary)

- Start and validate bridge mode:
  - `./scripts/validate-runtime.sh bridge`
- Confirm subscribers:
  - `podman exec vipertrade-redis redis-cli PUBSUB NUMSUB viper:market_data viper:decisions`
- Confirm strategy/executor activity:
  - `./scripts/compose.sh logs --tail 80 strategy`
  - `./scripts/compose.sh logs --tail 80 executor`

## Runtime Validation (Host Fallback)

- Start and validate host mode:
  - `./scripts/validate-runtime.sh host`
- Validate health:
  - `./scripts/health-check.sh`

## Rollback Plan

- DB rollback migration available:
  - `database/migrations/20260308_002_executor_fills_idempotency_down.sql`

- Stop current stack:
  - `./scripts/compose.sh down`
  - `./scripts/compose-host.sh down`
- Revert to last known-good commit:
  - `git checkout <known-good-sha>`
- Reapply env and bring stack up in host fallback:
  - `./scripts/compose-host.sh up -d`
  - `./scripts/health-check.sh`
- If bridge issue persists in WSL:
  - `./scripts/fix-podman-wsl-network.sh`

## Release Evidence

Final Phase 1 closure evidence (2026-03-08):

- Controlled testnet smoke passed (`DOGEUSDT` ENTER/CLOSE) with executor statuses `submitted` + `submitted_close`.
- Fill persistence verified in `bybit_fills` during smoke window.
- Idempotency check passed (`duplicate source_event_id = 0`).
- Manual smoke data cleanup executed after validation.

- Capture command outputs for:
  - `./scripts/ci-local.sh`
  - `./scripts/validate-runtime.sh bridge`
  - `./scripts/validate-runtime.sh host`
- Link passing GitHub CI run for release commit.
