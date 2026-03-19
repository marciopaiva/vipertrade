.DEFAULT_GOAL := help
SHELL := /bin/bash

# ═══════════════════════════════════════════════════════════════════════════
# CORES E FORMATAÇÃO
# ═══════════════════════════════════════════════════════════════════════════

GREEN  := \033[0;32m
YELLOW := \033[1;33m
RED    := \033[0;31m
BLUE   := \033[0;34m
CYAN   := \033[0;36m
MAGENTA:= \033[0;35m
NC     := \033[0m
MARGEM := 30

# ═══════════════════════════════════════════════════════════════════════════
# CABEÇALHO DO PROJETO
# ═══════════════════════════════════════════════════════════════════════════

define HEADER

             _____   .__   .__     __                   .__
            /  _  \  |  |  |  |  _/  |_   ____    ____  |  |__
           /  /_\  \ |  |  |  |  \   __\_/ __ \ _/ ___\ |  |  \
          /    |    \|  |__|  |__ |  |  \  ___/ \  \___ |   Y  \
          \____|__  /|____/|____/ |__|   \___  > \___  >|___|  /
                  \/                         \/      \/      \/

            VIPERTRADE - Lead Trader Bot for Bybit Copy Trading
                      Engine: TupaLang v0.8.0 | Rust 1.83

endef
export HEADER

# ═══════════════════════════════════════════════════════════════════════════
# VARIÁVEIS GLOBAIS
# ═══════════════════════════════════════════════════════════════════════════

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

.PHONY: help

help:  ## Exibir esta mensagem de ajuda
	@clear
	@printf "\033c"
	@printf "$$HEADER\n"
	@printf "\n"
	@printf "$(YELLOW)ViperTrade Makefile - Automação de Tarefas$(NC)\n\n"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		sed 's/:.*## / - /' | \
		while IFS='-' read -r target desc; do \
			printf "$(CYAN)%-$(MARGEM)s$(NC) %s\n" "$$target" "$$desc"; \
		done
	@printf "\n"
	@printf "Uso: $(GREEN)make$(NC) $(BLUE)[alvo]$(NC)\n"
	@printf "Exemplo: $(GREEN)make$(NC) $(BLUE)build$(NC), $(GREEN)make$(NC) $(BLUE)up$(NC), $(GREEN)make$(NC) $(BLUE)test$(NC)\n\n"

# ═══════════════════════════════════════════════════════════════════════════
# SETUP INICIAL
# ═══════════════════════════════════════════════════════════════════════════

.PHONY: setup setup-env setup-secrets setup-db

setup: setup-env setup-secrets setup-db  ## Setup inicial completo do ambiente

setup-env:  ## Copia arquivos de ambiente (.env.example → .env)
	@printf "$(YELLOW)→$(NC) Copiando arquivos de ambiente...\n"
	cp -n compose/.env.example compose/.env || true
	@printf "$(GREEN)✓$(NC) Arquivos de ambiente copiados\n"

setup-secrets:  ## Inicializa secrets do ambiente
	@printf "$(YELLOW)→$(NC) Inicializando secrets...\n"
	./scripts/init-secrets.sh
	@printf "$(GREEN)✓$(NC) Secrets inicializados\n"

setup-db:  ## Inicializa database com schema e migrations
	@printf "$(YELLOW)→$(NC) Inicializando database...\n"
	$(COMPOSE) up -d postgres
	@sleep 5
	@printf "$(GREEN)✓$(NC) Database inicializada\n"

# ═══════════════════════════════════════════════════════════════════════════
# BUILD
# ═══════════════════════════════════════════════════════════════════════════

.PHONY: build build-rust build-rust-release build-web build-images build-base-images

build: build-rust build-web  ## Build completo do projeto

build-rust:  ## Build do código Rust (workspace completo)
	@printf "$(YELLOW)→$(NC) Build Rust...\n"
	$(CARGO) build --workspace --locked

build-rust-release:  ## Build Rust em modo release (otimizado)
	@printf "$(YELLOW)→$(NC) Build Rust (release)...\n"
	$(CARGO) build --workspace --release --locked

build-web:  ## Build do frontend web (Next.js)
	@printf "$(YELLOW)→$(NC) Build Web...\n"
	cd services/web && $(YARN) install --frozen-lockfile
	cd services/web && $(YARN) build

build-images:  ## Build das imagens Docker
	@printf "$(YELLOW)→$(NC) Build Docker images...\n"
	./scripts/build-base-images.sh
	$(COMPOSE) build

build-base-images:  ## Build apenas das imagens base (Rust builder/runtime)
	@printf "$(YELLOW)→$(NC) Build base images...\n"
	./scripts/build-base-images.sh

# ═══════════════════════════════════════════════════════════════════════════
# DOCKER COMPOSE
# ═══════════════════════════════════════════════════════════════════════════

.PHONY: up up-host down down-timeout restart ps logs logs-strategy logs-executor logs-market-data logs-api

up:  ## Sobe todos os serviços em background
	@printf "$(YELLOW)→$(NC) Subindo serviços...\n"
	$(COMPOSE) up -d
	@printf "$(GREEN)✓$(NC) Serviços iniciados\n"

up-host:  ## Sobe serviços em modo host (fallback WSL)
	@printf "$(YELLOW)→$(NC) Subindo serviços (host mode)...\n"
	$(COMPOSE_HOST) up -d
	@printf "$(GREEN)✓$(NC) Serviços iniciados (host mode)\n"

down:  ## Derruba todos os serviços
	@printf "$(YELLOW)→$(NC) Derrubando serviços...\n"
	$(COMPOSE) down
	@printf "$(GREEN)✓$(NC) Serviços derrubados\n"

down-timeout:  ## Derruba serviços com timeout (30s)
	@printf "$(YELLOW)→$(NC) Derrubando serviços (timeout 30s)...\n"
	COMPOSE_DOWN_TIMEOUT=30 $(COMPOSE) down
	@printf "$(GREEN)✓$(NC) Serviços derrubados\n"

restart: down up  ## Reinicia todos os serviços

ps:  ## Mostra status dos serviços
	@printf "$(YELLOW)→$(NC) Status dos serviços:\n"
	$(COMPOSE) ps

logs:  ## Mostra logs de todos os serviços
	$(COMPOSE) logs -f

logs-strategy:  ## Mostra logs do serviço strategy
	$(COMPOSE) logs -f strategy

logs-executor:  ## Mostra logs do serviço executor
	$(COMPOSE) logs -f executor

logs-market-data:  ## Mostra logs do serviço market-data
	$(COMPOSE) logs -f market-data

logs-api:  ## Mostra logs do serviço api
	$(COMPOSE) logs -f api

# ═══════════════════════════════════════════════════════════════════════════
# TESTS
# ═══════════════════════════════════════════════════════════════════════════

.PHONY: test test-rust test-rust-lib test-rust-watch test-web

test: test-rust test-web  ## Roda todos os testes

test-rust:  ## Roda testes Rust
	@printf "$(YELLOW)→$(NC) Testes Rust...\n"
	$(CARGO) test --workspace --locked

test-rust-lib:  ## Roda apenas testes de bibliotecas (sem bins)
	@printf "$(YELLOW)→$(NC) Testes Rust (libs)...\n"
	$(CARGO) test --workspace --lib --locked

test-rust-watch:  ## Roda testes Rust em watch mode (requer cargo-watch)
	@printf "$(YELLOW)→$(NC) Testes Rust (watch)...\n"
	$(CARGO) watch test --workspace

test-web:  ## Roda testes do frontend web
	@printf "$(YELLOW)→$(NC) Testes Web...\n"
	cd services/web && $(YARN) test

# ═══════════════════════════════════════════════════════════════════════════
# LINT & FORMAT
# ═══════════════════════════════════════════════════════════════════════════

.PHONY: lint lint-rust lint-rust-check lint-web lint-docs format format-rust format-web check-format check-format-rust check-format-web

lint: lint-rust lint-web lint-docs  ## Roda todos os linters

lint-rust:  ## Roda linter Rust (clippy)
	@printf "$(YELLOW)→$(NC) Lint Rust...\n"
	$(CARGO) clippy --workspace --all-targets -- -D warnings

lint-rust-check:  ## Roda check Rust (sem clippy)
	@printf "$(YELLOW)→$(NC) Check Rust...\n"
	$(CARGO) check --workspace --locked

lint-web:  ## Roda linter do frontend web (ESLint)
	@printf "$(YELLOW)→$(NC) Lint Web...\n"
	cd services/web && $(YARN) lint

lint-docs:  ## Roda linter de documentação (markdownlint)
	@printf "$(YELLOW)→$(NC) Lint Docs...\n"
	./scripts/ci-local.sh

format: format-rust format-web  ## Formata todo o código

format-rust:  ## Formata código Rust
	@printf "$(YELLOW)→$(NC) Format Rust...\n"
	$(CARGO) fmt --all

format-web:  ## Formata código do frontend web
	@printf "$(YELLOW)→$(NC) Format Web...\n"
	cd services/web && $(YARN) format

check-format: check-format-rust check-format-web  ## Verifica formatação

check-format-rust:  ## Verifica formatação Rust
	@printf "$(YELLOW)→$(NC) Check format Rust...\n"
	$(CARGO) fmt --all -- --check

check-format-web:  ## Verifica formatação Web
	@printf "$(YELLOW)→$(NC) Check format Web...\n"
	cd services/web && $(YARN) format:check

# ═══════════════════════════════════════════════════════════════════════════
# VALIDATION
# ═══════════════════════════════════════════════════════════════════════════

.PHONY: validate validate-workspace validate-db validate-pipeline validate-runtime validate-runtime-host

validate: validate-workspace validate-db validate-pipeline validate-runtime  ## Validações completas

validate-workspace:  ## Valida workspace completo
	@printf "$(YELLOW)→$(NC) Validando workspace...\n"
	./scripts/validate-workspace.sh

validate-db:  ## Valida database e conexões
	@printf "$(YELLOW)→$(NC) Validando database...\n"
	./scripts/validate-db.sh

validate-pipeline:  ## Valida pipelines TupaLang
	@printf "$(YELLOW)→$(NC) Validando pipelines...\n"
	./scripts/validate-pipeline.sh

validate-runtime:  ## Valida runtime (bridge mode)
	@printf "$(YELLOW)→$(NC) Validando runtime (bridge)...\n"
	./scripts/validate-runtime.sh bridge

validate-runtime-host:  ## Valida runtime (host mode)
	@printf "$(YELLOW)→$(NC) Validando runtime (host)...\n"
	./scripts/validate-runtime.sh host

# ═══════════════════════════════════════════════════════════════════════════
# HEALTH CHECKS
# ═══════════════════════════════════════════════════════════════════════════

.PHONY: health health-postgres health-redis health-strategy health-executor health-api health-web

health:  ## Roda health checks de todos os serviços
	@printf "$(YELLOW)→$(NC) Health checks...\n"
	./scripts/health-check.sh

health-postgres:  ## Verifica saúde do PostgreSQL
	@printf "$(YELLOW)→$(NC) Health: PostgreSQL...\n"
	$(DOCKER) exec vipertrade-postgres pg_isready -U $(DB_USER) -d $(DB_NAME)

health-redis:  ## Verifica saúde do Redis
	@printf "$(YELLOW)→$(NC) Health: Redis...\n"
	$(DOCKER) exec vipertrade-redis redis-cli ping

health-strategy:  ## Verifica saúde do serviço strategy
	@printf "$(YELLOW)→$(NC) Health: Strategy...\n"
	curl -f http://localhost:8082/health || printf "$(RED)✗$(NC) Strategy não disponível\n"

health-executor:  ## Verifica saúde do serviço executor
	@printf "$(YELLOW)→$(NC) Health: Executor...\n"
	curl -f http://localhost:8083/health || printf "$(RED)✗$(NC) Executor não disponível\n"

health-api:  ## Verifica saúde do serviço api
	@printf "$(YELLOW)→$(NC) Health: API...\n"
	curl -f http://localhost:8080/health || printf "$(RED)✗$(NC) API não disponível\n"

health-web:  ## Verifica saúde do serviço web
	@printf "$(YELLOW)→$(NC) Health: Web...\n"
	curl -f http://localhost:3000 || printf "$(RED)✗$(NC) Web não disponível\n"

# ═══════════════════════════════════════════════════════════════════════════
# DATABASE
# ═══════════════════════════════════════════════════════════════════════════

.PHONY: db-migrate db-reset db-backup db-restore db-shell db-truncate

db-migrate:  ## Roda migrations do database
	@printf "$(YELLOW)→$(NC) Rodando migrations...\n"
	sqlx migrate run --database-url postgresql://$(DB_USER)@$(DB_HOST):$(DB_PORT)/$(DB_NAME)

db-reset:  ## Reseta paper database (cuidado: apaga dados!)
	@printf "$(RED)→$(NC) Resetando paper database...\n"
	./scripts/reset-paper-db.sh

db-backup:  ## Backup do database
	@printf "$(YELLOW)→$(NC) Backup database...\n"
	$(DOCKER) exec vipertrade-postgres pg_dump -U $(DB_USER) $(DB_NAME) > backup_$$(date +%Y%m%d_%H%M%S).sql
	@printf "$(GREEN)✓$(NC) Backup criado: backup_$$(date +%Y%m%d_%H%M%S).sql\n"

db-restore:  ## Restore do database (requer arquivo .sql)
	@printf "$(YELLOW)→$(NC) Restore database...\n"
	@printf "Uso: make db-restore FILE=backup_20260319_120000.sql\n"
	$(DOCKER) exec -i vipertrade-postgres psql -U $(DB_USER) -d $(DB_NAME) < $(FILE)

db-shell:  ## Acessa shell do PostgreSQL
	@printf "$(YELLOW)→$(NC) PostgreSQL shell...\n"
	$(DOCKER) exec -it vipertrade-postgres psql -U $(DB_USER) -d $(DB_NAME)

db-truncate:  ## Trunca todas as tabelas (cuidado: apaga dados!)
	@printf "$(RED)→$(NC) Truncando tabelas...\n"
	$(DOCKER) exec vipertrade-postgres psql -U $(DB_USER) -d $(DB_NAME) -c \
		"TRUNCATE TABLE trades, system_events, bybit_fills, position_snapshots RESTART IDENTITY CASCADE;"

# ═══════════════════════════════════════════════════════════════════════════
# TRADING OPERATIONS
# ═══════════════════════════════════════════════════════════════════════════

.PHONY: trading-status kill-switch kill-switch-reset phase4-validate phase5-validate phase6-validate

trading-status:  ## Mostra status atual do trading
	@printf "$(YELLOW)→$(NC) Trading Status:\n"
	@printf "  $(CYAN)TRADING_MODE:$(NC) $(TRADING_MODE)\n"
	@printf "  $(CYAN)BYBIT_ENV:$(NC) $(BYBIT_ENV)\n"
	@printf "  $(CYAN)LOG_LEVEL:$(NC) $(LOG_LEVEL)\n"
	@curl -s http://localhost:8080/api/v1/status 2>/dev/null | jq . || printf "  $(RED)API não disponível$(NC)\n"

kill-switch:  ## Ativa kill switch (para trading)
	@printf "$(RED)→$(NC) Ativando kill switch...\n"
	./scripts/kill-switch-control.sh activate

kill-switch-reset:  ## Desativa kill switch (retoma trading)
	@printf "$(GREEN)→$(NC) Desativando kill switch...\n"
	./scripts/kill-switch-control.sh reset

phase4-validate:  ## Validação Phase 4 (Backtest/Paper)
	@printf "$(YELLOW)→$(NC) Phase 4 Validation...\n"
	./scripts/phase4-validate.sh

phase5-validate:  ## Validação Phase 5 (Smart Copy/Trailing)
	@printf "$(YELLOW)→$(NC) Phase 5 Validation...\n"
	./scripts/phase5-validate.sh

phase6-validate:  ## Validação Phase 6 (Mainnet Readiness)
	@printf "$(YELLOW)→$(NC) Phase 6 Validation...\n"
	./scripts/phase6-validate.sh

# ═══════════════════════════════════════════════════════════════════════════
# SECURITY
# ═══════════════════════════════════════════════════════════════════════════

.PHONY: security-check audit-deps audit-outdated

security-check:  ## Roda check de segurança
	@printf "$(YELLOW)→$(NC) Security check...\n"
	./scripts/security-check.sh

audit-deps:  ## Auditoria de dependências Rust
	@printf "$(YELLOW)→$(NC) Audit dependencies...\n"
	$(CARGO) audit

audit-outdated:  ## Verifica dependências desatualizadas
	@printf "$(YELLOW)→$(NC) Check outdated dependencies...\n"
	$(CARGO) outdated

# ═══════════════════════════════════════════════════════════════════════════
# CI/CD
# ═══════════════════════════════════════════════════════════════════════════

.PHONY: ci-local ci-strict pre-commit

ci-local:  ## Roda CI localmente (paridade com GitHub Actions)
	@printf "$(YELLOW)→$(NC) CI Local...\n"
	./scripts/ci-local.sh

ci-strict:  ## Roda CI local em modo strict (com docs)
	@printf "$(YELLOW)→$(NC) CI Local (strict)...\n"
	CI_LOCAL_STRICT_DOCS=1 ./scripts/ci-local.sh

pre-commit: format lint test  ## Roda hooks de pre-commit

# ═══════════════════════════════════════════════════════════════════════════
# CLEANUP
# ═══════════════════════════════════════════════════════════════════════════

.PHONY: clean clean-rust clean-web clean-docker clean-all

clean: clean-rust clean-web  ## Limpa artefatos de build

clean-rust:  ## Limpa artefatos Rust
	@printf "$(YELLOW)→$(NC) Clean Rust...\n"
	$(CARGO) clean

clean-web:  ## Limpa artefatos Web
	@printf "$(YELLOW)→$(NC) Clean Web...\n"
	cd services/web && rm -rf .next node_modules

clean-docker:  ## Limpa containers e volumes Docker
	@printf "$(YELLOW)→$(NC) Clean Docker...\n"
	$(DOCKER) system prune -f
	$(DOCKER) volume prune -f

clean-all: clean clean-docker  ## Limpeza completa

# ═══════════════════════════════════════════════════════════════════════════
# DESENVOLVIMENTO
# ═══════════════════════════════════════════════════════════════════════════

.PHONY: dev dev-rust dev-web

dev: up logs  ## Modo desenvolvimento (sobe serviços e mostra logs)

dev-rust:  ## Roda cargo watch para desenvolvimento Rust
	@printf "$(YELLOW)→$(NC) Dev Rust (watch)...\n"
	$(CARGO) watch -x check --workspace

dev-web:  ## Roda dev server do frontend web
	@printf "$(YELLOW)→$(NC) Dev Web...\n"
	cd services/web && $(YARN) dev

# ═══════════════════════════════════════════════════════════════════════════
# DOCUMENTAÇÃO
# ═══════════════════════════════════════════════════════════════════════════

.PHONY: docs docs-open

docs:  ## Gera documentação
	@printf "$(YELLOW)→$(NC) Gerando documentação...\n"
	$(CARGO) doc --workspace --no-deps

docs-open: docs  ## Gera e abre documentação
	@printf "$(YELLOW)→$(NC) Abrindo documentação...\n"
	$(CARGO) doc --workspace --no-deps --open

# ═══════════════════════════════════════════════════════════════════════════
# UTILITÁRIOS
# ═══════════════════════════════════════════════════════════════════════════

.PHONY: git-hooks env version

git-hooks:  ## Setup git hooks
	@printf "$(YELLOW)→$(NC) Setup git hooks...\n"
	./scripts/setup-git-hooks.sh

env:  ## Mostra variáveis de ambiente atuais
	@printf "$(YELLOW)→$(NC) Variáveis de ambiente:\n"
	@printf "  $(CYAN)COMPOSE:$(NC) $(COMPOSE)\n"
	@printf "  $(CYAN)CARGO:$(NC) $(CARGO)\n"
	@printf "  $(CYAN)YARN:$(NC) $(YARN)\n"
	@printf "  $(CYAN)TRADING_MODE:$(NC) $(TRADING_MODE)\n"
	@printf "  $(CYAN)BYBIT_ENV:$(NC) $(BYBIT_ENV)\n"
	@printf "  $(CYAN)LOG_LEVEL:$(NC) $(LOG_LEVEL)\n"

version:  ## Mostra versões das ferramentas
	@printf "$(YELLOW)→$(NC) Versões:\n"
	@printf "  $(CYAN)Rust:$(NC) $$($(CARGO) --version)\n"
	@printf "  $(CYAN)Node:$(NC) $$($(YARN) --version 2>/dev/null || echo 'não instalado')\n"
	@printf "  $(CYAN)Docker:$(NC) $$($(DOCKER) --version 2>/dev/null || echo 'não instalado')\n"
	@printf "  $(CYAN)Docker Compose:$(NC) $$($(DOCKER) compose version 2>/dev/null || echo 'não instalado')\n"

# ═══════════════════════════════════════════════════════════════════════════
# FIM DO MAKEFILE
# ═══════════════════════════════════════════════════════════════════════════
