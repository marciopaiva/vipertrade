# ViperTrade Makefile
# Makefile para automação de tarefas de desenvolvimento e operações
#
# Uso: make <target>
# Exemplo: make build, make test, make up

# ═══════════════════════════════════════════════════════════════════════════
# VARIÁVEIS GLOBAIS
# ═══════════════════════════════════════════════════════════════════════════

.PHONY: help

# Docker/Compose
COMPOSE ?= ./scripts/compose.sh
COMPOSE_HOST ?= ./scripts/compose-host.sh
DOCKER := docker

# Rust
CARGO := cargo
RUST_VERSION := 1.83

# Node/Web
YARN := yarn
NODE_VERSION := 20

# Database
DB_HOST ?= localhost
DB_PORT ?= 5432
DB_NAME ?= vipertrade
DB_USER ?= viper

# Trading Mode
TRADING_MODE ?= paper
BYBIT_ENV ?= testnet

# Logs
LOG_LEVEL ?= info

# ═══════════════════════════════════════════════════════════════════════════
# HELP (alvo padrão)
# ═══════════════════════════════════════════════════════════════════════════

help: ## Mostra ajuda com todos os targets disponíveis
	@echo "ViperTrade Makefile"
	@echo "==================="
	@echo ""
	@echo "Uso: make <target>"
	@echo ""
	@echo "Targets principais:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-25s\033[0m %s\n", $$1, $$2}'
	@echo ""

# ═══════════════════════════════════════════════════════════════════════════
# SETUP INICIAL
# ═══════════════════════════════════════════════════════════════════════════

setup: setup-env setup-secrets setup-db ## Setup inicial completo do ambiente

setup-env: ## Copia arquivos de ambiente (.env.example → .env)
	@echo "→ Copiando arquivos de ambiente..."
	cp -n compose/.env.example compose/.env || true
	@echo "✅ Arquivos de ambiente copiados"

setup-secrets: ## Inicializa secrets do ambiente
	@echo "→ Inicializando secrets..."
	./scripts/init-secrets.sh
	@echo "✅ Secrets inicializados"

setup-db: ## Inicializa database com schema e migrations
	@echo "→ Inicializando database..."
	$(COMPOSE) up -d postgres
	@sleep 5
	@echo "✅ Database inicializada"

# ═══════════════════════════════════════════════════════════════════════════
# BUILD
# ═══════════════════════════════════════════════════════════════════════════

build: build-rust build-web ## Build completo do projeto

build-rust: ## Build do código Rust (workspace completo)
	@echo "→ Build Rust..."
	$(CARGO) build --workspace --locked

build-rust-release: ## Build Rust em modo release (otimizado)
	@echo "→ Build Rust (release)..."
	$(CARGO) build --workspace --release --locked

build-web: ## Build do frontend web (Next.js)
	@echo "→ Build Web..."
	cd services/web && $(YARN) install --frozen-lockfile
	cd services/web && $(YARN) build

build-images: ## Build das imagens Docker
	@echo "→ Build Docker images..."
	./scripts/build-base-images.sh
	$(COMPOSE) build

build-base-images: ## Build apenas das imagens base (Rust builder/runtime)
	@echo "→ Build base images..."
	./scripts/build-base-images.sh

# ═══════════════════════════════════════════════════════════════════════════
# DOCKER COMPOSE
# ═══════════════════════════════════════════════════════════════════════════

up: ## Sobe todos os serviços em background
	@echo "→ Subindo serviços..."
	$(COMPOSE) up -d

up-host: ## Sobe serviços em modo host (fallback WSL)
	@echo "→ Subindo serviços (host mode)..."
	$(COMPOSE_HOST) up -d

down: ## Derruba todos os serviços
	@echo "→ Derrubando serviços..."
	$(COMPOSE) down

down-timeout: ## Derruba serviços com timeout (30s)
	@echo "→ Derrubando serviços (timeout 30s)..."
	COMPOSE_DOWN_TIMEOUT=30 $(COMPOSE) down

restart: down up ## Reinicia todos os serviços

ps: ## Mostra status dos serviços
	@echo "→ Status dos serviços:"
	$(COMPOSE) ps

logs: ## Mostra logs de todos os serviços
	$(COMPOSE) logs -f

logs-strategy: ## Mostra logs do serviço strategy
	$(COMPOSE) logs -f strategy

logs-executor: ## Mostra logs do serviço executor
	$(COMPOSE) logs -f executor

logs-market-data: ## Mostra logs do serviço market-data
	$(COMPOSE) logs -f market-data

logs-api: ## Mostra logs do serviço api
	$(COMPOSE) logs -f api

# ═══════════════════════════════════════════════════════════════════════════
# TESTS
# ═══════════════════════════════════════════════════════════════════════════

test: test-rust test-web ## Roda todos os testes

test-rust: ## Roda testes Rust
	@echo "→ Testes Rust..."
	$(CARGO) test --workspace --locked

test-rust-lib: ## Roda apenas testes de bibliotecas (sem bins)
	@echo "→ Testes Rust (libs)..."
	$(CARGO) test --workspace --lib --locked

test-rust-watch: ## Roda testes Rust em watch mode (requer cargo-watch)
	@echo "→ Testes Rust (watch)..."
	$(CARGO) watch test --workspace

test-web: ## Roda testes do frontend web
	@echo "→ Testes Web..."
	cd services/web && $(YARN) test

# ═══════════════════════════════════════════════════════════════════════════
# LINT & FORMAT
# ═══════════════════════════════════════════════════════════════════════════

lint: lint-rust lint-web lint-docs ## Roda todos os linters

lint-rust: ## Roda linter Rust (clippy)
	@echo "→ Lint Rust..."
	$(CARGO) clippy --workspace --all-targets -- -D warnings

lint-rust-check: ## Roda check Rust (sem clippy)
	@echo "→ Check Rust..."
	$(CARGO) check --workspace --locked

lint-web: ## Roda linter do frontend web (ESLint)
	@echo "→ Lint Web..."
	cd services/web && $(YARN) lint

lint-docs: ## Roda linter de documentação (markdownlint)
	@echo "→ Lint Docs..."
	./scripts/ci-local.sh

format: format-rust format-web ## Formata todo o código

format-rust: ## Formata código Rust
	@echo "→ Format Rust..."
	$(CARGO) fmt --all

format-web: ## Formata código do frontend web
	@echo "→ Format Web..."
	cd services/web && $(YARN) format

check-format: check-format-rust check-format-web ## Verifica formatação

check-format-rust: ## Verifica formatação Rust
	@echo "→ Check format Rust..."
	$(CARGO) fmt --all -- --check

check-format-web: ## Verifica formatação Web
	@echo "→ Check format Web..."
	cd services/web && $(YARN) format:check

# ═══════════════════════════════════════════════════════════════════════════
# VALIDATION
# ═══════════════════════════════════════════════════════════════════════════

validate: validate-workspace validate-db validate-pipeline validate-runtime ## Validações completas

validate-workspace: ## Valida workspace completo
	@echo "→ Validando workspace..."
	./scripts/validate-workspace.sh

validate-db: ## Valida database e conexões
	@echo "→ Validando database..."
	./scripts/validate-db.sh

validate-pipeline: ## Valida pipelines TupaLang
	@echo "→ Validando pipelines..."
	./scripts/validate-pipeline.sh

validate-runtime: ## Valida runtime (bridge mode)
	@echo "→ Validando runtime..."
	./scripts/validate-runtime.sh bridge

validate-runtime-host: ## Valida runtime (host mode)
	@echo "→ Validando runtime (host)..."
	./scripts/validate-runtime.sh host

# ═══════════════════════════════════════════════════════════════════════════
# HEALTH CHECKS
# ═══════════════════════════════════════════════════════════════════════════

health: ## Roda health checks de todos os serviços
	@echo "→ Health checks..."
	./scripts/health-check.sh

health-postgres: ## Verifica saúde do PostgreSQL
	@echo "→ Health: PostgreSQL..."
	$(DOCKER) exec vipertrade-postgres pg_isready -U $(DB_USER) -d $(DB_NAME)

health-redis: ## Verifica saúde do Redis
	@echo "→ Health: Redis..."
	$(DOCKER) exec vipertrade-redis redis-cli ping

health-strategy: ## Verifica saúde do serviço strategy
	@echo "→ Health: Strategy..."
	curl -f http://localhost:8082/health || echo "Strategy não disponível"

health-executor: ## Verifica saúde do serviço executor
	@echo "→ Health: Executor..."
	curl -f http://localhost:8083/health || echo "Executor não disponível"

health-api: ## Verifica saúde do serviço api
	@echo "→ Health: API..."
	curl -f http://localhost:8080/health || echo "API não disponível"

health-web: ## Verifica saúde do serviço web
	@echo "→ Health: Web..."
	curl -f http://localhost:3000 || echo "Web não disponível"

# ═══════════════════════════════════════════════════════════════════════════
# DATABASE
# ═══════════════════════════════════════════════════════════════════════════

db-migrate: ## Roda migrations do database
	@echo "→ Rodando migrations..."
	sqlx migrate run --database-url postgresql://$(DB_USER)@$(DB_HOST):$(DB_PORT)/$(DB_NAME)

db-reset: ## Reseta paper database (cuidado: apaga dados!)
	@echo "→ Resetando paper database..."
	./scripts/reset-paper-db.sh

db-backup: ## Backup do database
	@echo "→ Backup database..."
	$(DOCKER) exec vipertrade-postgres pg_dump -U $(DB_USER) $(DB_NAME) > backup_$(shell date +%Y%m%d_%H%M%S).sql

db-restore: ## Restore do database (requer arquivo .sql)
	@echo "→ Restore database..."
	@echo "Uso: make db-restore FILE=backup_20260319_120000.sql"
	$(DOCKER) exec -i vipertrade-postgres psql -U $(DB_USER) -d $(DB_NAME) < $(FILE)

db-shell: ## Acessa shell do PostgreSQL
	@echo "→ PostgreSQL shell..."
	$(DOCKER) exec -it vipertrade-postgres psql -U $(DB_USER) -d $(DB_NAME)

db-truncate: ## Trunca todas as tabelas (cuidado: apaga dados!)
	@echo "→ Truncando tabelas..."
	$(DOCKER) exec vipertrade-postgres psql -U $(DB_USER) -d $(DB_NAME) -c \
		"TRUNCATE TABLE trades, system_events, bybit_fills, position_snapshots RESTART IDENTITY CASCADE;"

# ═══════════════════════════════════════════════════════════════════════════
# TRADING OPERATIONS
# ═══════════════════════════════════════════════════════════════════════════

trading-status: ## Mostra status atual do trading
	@echo "→ Trading Status:"
	@echo "  TRADING_MODE: $(TRADING_MODE)"
	@echo "  BYBIT_ENV: $(BYBIT_ENV)"
	@echo "  LOG_LEVEL: $(LOG_LEVEL)"
	curl -s http://localhost:8080/api/v1/status 2>/dev/null | jq . || echo "API não disponível"

kill-switch: ## Ativa kill switch (para trading)
	@echo "→ Ativando kill switch..."
	./scripts/kill-switch-control.sh activate

kill-switch-reset: ## Desativa kill switch (retoma trading)
	@echo "→ Desativando kill switch..."
	./scripts/kill-switch-control.sh reset

phase4-validate: ## Validação Phase 4 (Backtest/Paper)
	@echo "→ Phase 4 Validation..."
	./scripts/phase4-validate.sh

phase5-validate: ## Validação Phase 5 (Smart Copy/Trailing)
	@echo "→ Phase 5 Validation..."
	./scripts/phase5-validate.sh

phase6-validate: ## Validação Phase 6 (Mainnet Readiness)
	@echo "→ Phase 6 Validation..."
	./scripts/phase6-validate.sh

# ═══════════════════════════════════════════════════════════════════════════
# SECURITY
# ═══════════════════════════════════════════════════════════════════════════

security-check: ## Roda check de segurança
	@echo "→ Security check..."
	./scripts/security-check.sh

audit-deps: ## Auditoria de dependências Rust
	@echo "→ Audit dependencies..."
	$(CARGO) audit

audit-outdated: ## Verifica dependências desatualizadas
	@echo "→ Check outdated dependencies..."
	$(CARGO) outdated

# ═══════════════════════════════════════════════════════════════════════════
# CI/CD
# ═══════════════════════════════════════════════════════════════════════════

ci-local: ## Roda CI localmente (paridade com GitHub Actions)
	@echo "→ CI Local..."
	./scripts/ci-local.sh

ci-strict: ## Roda CI local em modo strict (com docs)
	@echo "→ CI Local (strict)..."
	CI_LOCAL_STRICT_DOCS=1 ./scripts/ci-local.sh

pre-commit: format lint test ## Roda hooks de pre-commit

# ═══════════════════════════════════════════════════════════════════════════
# CLEANUP
# ═══════════════════════════════════════════════════════════════════════════

clean: clean-rust clean-web ## Limpa artefatos de build

clean-rust: ## Limpa artefatos Rust
	@echo "→ Clean Rust..."
	$(CARGO) clean

clean-web: ## Limpa artefatos Web
	@echo "→ Clean Web..."
	cd services/web && rm -rf .next node_modules

clean-docker: ## Limpa containers e volumes Docker
	@echo "→ Clean Docker..."
	$(DOCKER) system prune -f
	$(DOCKER) volume prune -f

clean-all: clean clean-docker ## Limpeza completa

# ═══════════════════════════════════════════════════════════════════════════
# DESENVOLVIMENTO
# ═══════════════════════════════════════════════════════════════════════════

dev: up logs ## Modo desenvolvimento (sobe serviços e mostra logs)

dev-rust: ## Roda cargo watch para desenvolvimento Rust
	@echo "→ Dev Rust (watch)..."
	$(CARGO) watch -x check --workspace

dev-web: ## Roda dev server do frontend web
	@echo "→ Dev Web..."
	cd services/web && $(YARN) dev

# ═══════════════════════════════════════════════════════════════════════════
# DOCUMENTAÇÃO
# ═══════════════════════════════════════════════════════════════════════════

docs: ## Gera documentação
	@echo "→ Gerando documentação..."
	$(CARGO) doc --workspace --no-deps

docs-open: docs ## Gera e abre documentação
	@echo "→ Abrindo documentação..."
	$(CARGO) doc --workspace --no-deps --open

# ═══════════════════════════════════════════════════════════════════════════
# UTILITÁRIOS
# ═══════════════════════════════════════════════════════════════════════════

git-hooks: ## Setup git hooks
	@echo "→ Setup git hooks..."
	./scripts/setup-git-hooks.sh

env: ## Mostra variáveis de ambiente atuais
	@echo "Variáveis de ambiente:"
	@echo "  COMPOSE: $(COMPOSE)"
	@echo "  CARGO: $(CARGO)"
	@echo "  YARN: $(YARN)"
	@echo "  TRADING_MODE: $(TRADING_MODE)"
	@echo "  BYBIT_ENV: $(BYBIT_ENV)"
	@echo "  LOG_LEVEL: $(LOG_LEVEL)"

version: ## Mostra versões das ferramentas
	@echo "Versões:"
	@echo "  Rust: $$($(CARGO) --version)"
	@echo "  Node: $$($(YARN) --version 2>/dev/null || echo 'não instalado')"
	@echo "  Docker: $$($(DOCKER) --version 2>/dev/null || echo 'não instalado')"
	@echo "  Docker Compose: $$($(DOCKER) compose version 2>/dev/null || echo 'não instalado')"

# ═══════════════════════════════════════════════════════════════════════════
# FIM DO MAKEFILE
# ═══════════════════════════════════════════════════════════════════════════
