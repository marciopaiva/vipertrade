# 🐍 ViperTrade

Lead Trader bot para Bybit Copy Trading Classic com engine Tupa.

## Stack

- Rust microservices (market-data, strategy, executor, monitor, backtest, api)
- PostgreSQL + Redis
- Web dashboard (Next.js)
- Orquestracao com Podman Compose

## Ambiente recomendado (WSL Fedora + Podman)

### Pre-requisitos

- WSL Fedora
- Podman + podman-compose (via scripts/compose.sh)
- Git

### Setup rapido

```bash
git clone https://github.com/marciopaiva/vipertrade.git
cd vipertrade
cp compose/.env.example compose/.env
./scripts/init-secrets.sh
./scripts/security-check.sh
```

### Subir ambiente

```bash
./scripts/compose.sh up -d
```

### Modo host local (fallback de emergencia)

```bash
./scripts/compose-host.sh up -d
./scripts/health-check.sh
```

Para parar:

```bash
./scripts/compose-host.sh down
```

### Corrigir rede bridge no WSL

```bash
./scripts/fix-podman-wsl-network.sh
```

### Validar runtime end-to-end

```bash
./scripts/validate-runtime.sh bridge
# fallback local
./scripts/validate-runtime.sh host
```

### Validar saude

```bash
./scripts/health-check.sh
```

### Logs uteis

```bash
./scripts/compose.sh logs -f strategy
./scripts/compose.sh logs -f market-data
./scripts/compose.sh logs -f api
```

### Parar ambiente

```bash
./scripts/compose.sh down
```

Opcional (timeout de shutdown):

```bash
COMPOSE_DOWN_TIMEOUT=20 ./scripts/compose.sh down
```

## CI

GitHub Actions ativo em PR/push:

- Rust: `cargo fmt --check` + `cargo check --workspace --locked`
- Web: `yarn install --frozen-lockfile` + `yarn build`

Workflow: `.github/workflows/ci.yml`

## Documentacao

- Spec modular index: `docs/spec/README.md`
- Spec modules:
  - `docs/spec/01-overview.md`
  - `docs/spec/02-architecture.md`
  - `docs/spec/03-risk-and-profiles.md`
  - `docs/spec/04-bybit-integration.md`
  - `docs/spec/05-runtime-and-operations.md`
  - `docs/spec/06-validation-and-checklists.md`
- Operations runbook: `docs/operations/RUNBOOK.md`
- Event contract schema: `docs/contracts/strategy-decision-event-v1.schema.json`
- Legacy spec: `VIPERTRADE_SPEC.md`
- Architecture: `docs/ARCHITECTURE_V2.md`
- Phase plans: `docs/PHASE1_PLAN.md`, `docs/PHASE2_RISK_RECON_PLAN.md`

## Quality Gates

- Full local validation report:
  - `./scripts/validate-workspace.sh`
- Strict local CI parity:
  - `CI_LOCAL_STRICT_DOCS=1 ./scripts/ci-local.sh`

## Status atual (RC sem tag)

- Infra e servicos sobem com Podman Compose
- Health checks principais respondendo
- Bridge padrao validado no WSL com netavark + iptables


## Release Ops (0.8.0-rc)

Live testnet controls (executor):

- `EXECUTOR_ENABLE_LIVE_ORDERS=false` by default
- `EXECUTOR_LIVE_SYMBOL_ALLOWLIST=DOGEUSDT` for gradual rollout
- `EXECUTOR_RECONCILE_FIX=false` by default (detect/log)

Quick SQL checks after smoke cycle:

```bash
podman exec -i vipertrade-postgres psql -U viper -d vipertrade <<'SQL'
SELECT COUNT(*) AS fills_total FROM bybit_fills;
SELECT COUNT(*) AS duplicate_source_ids
FROM (
  SELECT data->>'source_event_id' sid, COUNT(*) c
  FROM system_events
  WHERE event_type='executor_event_processed'
    AND COALESCE(data->>'source_event_id','') <> ''
  GROUP BY data->>'source_event_id'
  HAVING COUNT(*) > 1
) t;
SQL
```
