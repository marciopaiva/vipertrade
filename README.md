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
- Bridge padrao validado no WSL com netavark + iptables
