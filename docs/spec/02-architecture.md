# 02 - Architecture

## Topology

- `market-data`
  - Bybit and multi-exchange market signal ingestion and normalization
- `strategy`
  - Tupa-driven strategy evaluation and decision generation
- `executor`
  - exchange-side execution path (paper/testnet/mainnet modes)
- `monitor`
  - health checks, alerting, and reconciliation
- `analytics`
  - market analysis and strategy performance insights
- `ai-analyst`
  - heuristic diagnostics and deterministic backtest sweep
- `api`
  - REST endpoints for status, trades, positions, controls
- `web`
  - operator dashboard with live runtime context
- `postgres`
  - durable runtime state (trades, positions, events)
- `redis`
  - pub/sub transport and transient state

## Services and responsibilities

- `postgres`
  - trades, position snapshots, events, and metrics
- `redis`
  - event transport and short-lived runtime state
- `api` and `web`
  - read and control surface for operators

## Decision flow

1. market data enters `market-data`
2. normalized events are published through `redis`
3. `strategy` parses, typechecks, and loads the Tupa-backed plan in-process, then combines that plan with runtime state to publish a decision
4. `executor` validates and executes the exchange-side action based on `TRADING_MODE` (paper/testnet/mainnet)
5. execution results are persisted and exposed to observability surfaces
6. `monitor` runs periodic reconciliation and drift checks

## Local runtime

- Podman rootless + WSL2 is the standard local environment
- bridge networking via `podman compose`
- `./scripts/compose.sh` is the wrapper for compose operations
- `./scripts/compose.sh up -d`, `./scripts/compose.sh down` for service lifecycle
- Kind cluster with `KIND_EXPERIMENTAL_PROVIDER=podman` for K8s development
- `compose/docker-compose.yml` is the only supported compose entrypoint
- service health checks are part of the default operational flow

## Kubernetes runtime

- Kind cluster with Podman experimental provider for development
- WSL2 + Podman rootless configuration
- Local registry at `localhost:5001` accessible via `kind` network
- See `k8s/kind/README.md` for deployment instructions
