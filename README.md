# ViperTrade v0.8.1

Lead Trader bot para Bybit Copy Trading Classic com engine Tupa.

## Stack
- Rust microservices (market-data, strategy, executor, monitor, backtest, api)
- PostgreSQL + Redis
- Web dashboard (Next.js)
- Orquestracao com Podman Compose

## Ambiente recomendado (WSL Fedora + Podman)

### Pre-requisitos
- WSL Fedora
- Podman + podman-compose
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
podman-compose -f compose/docker-compose.yml up -d
```

### Validar saude
```bash
./scripts/health-check.sh
```

### Logs uteis
```bash
podman logs -f vipertrade-strategy
podman logs -f vipertrade-market-data
podman logs -f vipertrade-api
```

### Parar ambiente
```bash
podman-compose -f compose/docker-compose.yml down
```

## CI
GitHub Actions ativo em PR/push:
- Rust: `cargo fmt --check` + `cargo check --workspace --locked`
- Web: `yarn install --frozen-lockfile` + `yarn build`

Workflow: `.github/workflows/ci.yml`

## Documentacao
- Especificacao: `VIPERTRADE_SPEC.md`
- Arquitetura: `docs/ARCHITECTURE_V2.md`
- Plano fase 1: `docs/PHASE1_PLAN.md`

## Status atual (RC sem tag)
- Infra e servicos sobem com Podman Compose
- Health checks principais respondendo
- Debito tecnico aberto: pipeline do strategy falhando parse (`Unexpected(Ident("type"))`)
