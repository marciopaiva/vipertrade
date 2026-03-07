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
- Capture command outputs for:
  - `./scripts/ci-local.sh`
  - `./scripts/validate-runtime.sh bridge`
  - `./scripts/validate-runtime.sh host`
- Link passing GitHub CI run for release commit.
