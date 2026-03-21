# 02 - Architecture

Source: `docs/legacy/VIPERTRADE_SPEC.md` (section 2).

## Topology

- `market-data`
  - Bybit and multi-exchange market signal ingestion and normalization
- `strategy`
  - Tupa-driven strategy evaluation and decision generation
- `executor`
  - exchange-side execution path
- `monitor`
  - health checks, alerting, and reconciliation
- `postgres`
  - durable runtime state
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
4. `executor` validates and executes the exchange-side action
5. execution results are persisted and exposed to observability surfaces
6. `monitor` runs periodic reconciliation and drift checks

## Local runtime

- Docker Desktop + WSL is the standard local environment
- bridge networking is the standard compose mode
- `compose/docker-compose.yml` is the only supported compose entrypoint
- service health checks are part of the default operational flow

## Original reference

- `docs/legacy/VIPERTRADE_SPEC.md`, approximate lines 87-161.
