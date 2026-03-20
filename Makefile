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

 __      __.__             ___________          __          .___
/  \    /  \__| ____  ____ \__    ___/___  ____/  |_  ____  |   |
\   \/\/   /  |/ ___\/ __ \ |    | /  _ \/  _ \   __\/ __ \ |   |
 \        /|  \  \__\  ___/ |    |(  <_> )  |_/|  | \  ___/ |   |
  \__/\  / |__|\___  >___  >|____| \____/|   / |__|  \___  >|___|
       \/          \/    \/              |__|            \/

         VIPERTRADE • Lead Trader Bot for Bybit Copy Trading
           TupaLang v0.8.0 • Rust 1.83 • Version 0.8.0-rc
═════════════════════════════════════════════════════════════════════
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

# Targets com ##@ são mostrados no help principal
# Targets com ## são mostrados apenas com make <target> help

help:  ## Exibir esta mensagem de ajuda
	@clear
	@printf "\033c"
	@printf "$$HEADER\n"
	@printf "\n"
	@printf "$(YELLOW)ViperTrade Makefile - Automação de Tarefas$(NC)\n\n"
	@printf "$(CYAN)Targets Principais (menus):$(NC)\n\n"
	@grep -E '^[a-zA-Z_-]+:.*?##@ .*$$' $(MAKEFILE_LIST) | \
		sed 's/^\([a-zA-Z_-]*\):.*##@ \(.*\)/\1|\2/' | \
		while IFS='|' read -r target desc; do \
			printf "$(GREEN)%-$(MARGEM)s$(NC) %s\n" "$$target" "$$desc"; \
		done
	@printf "\n"
	@printf "Uso: $(GREEN)make$(NC) $(BLUE)[alvo]$(NC)\n"
	@printf "Exemplo: $(GREEN)make$(NC) $(BLUE)build$(NC), $(GREEN)make$(NC) $(BLUE)up$(NC), $(GREEN)make$(NC) $(BLUE)health-all$(NC)\n\n"

# ═══════════════════════════════════════════════════════════════════════════
# HEALTH CHECKS
# ═══════════════════════════════════════════════════════════════════════════

.PHONY: health health-all health-postgres health-redis health-strategy health-executor health-api health-web

HEALTH_SERVICE ?= all

health:  ##@ Health Checks [ + ]
	@clear
	@printf "\033c"
	@printf "$$HEADER\n"
	@printf "\n"
	@printf "$(YELLOW)ViperTrade - Health Checks$(NC)\n\n"
	@printf "$(CYAN)make health-[serviço]$(NC)\n\n"
	@printf "Serviços disponíveis:\n"
	@printf "  $(CYAN)make health-all$(NC)        - Todos os serviços\n"
	@printf "  $(CYAN)make health-postgres$(NC)   - PostgreSQL\n"
	@printf "  $(CYAN)make health-redis$(NC)      - Redis\n"
	@printf "  $(CYAN)make health-market-data$(NC) - Market Data (8081)\n"
	@printf "  $(CYAN)make health-analytics$(NC)  - Analytics (8086)\n"
	@printf "  $(CYAN)make health-strategy$(NC)   - Strategy (8082)\n"
	@printf "  $(CYAN)make health-executor$(NC)   - Executor (8083)\n"
	@printf "  $(CYAN)make health-monitor$(NC)    - Monitor (8084)\n"
	@printf "  $(CYAN)make health-backtest$(NC)   - Backtest (8085)\n"
	@printf "  $(CYAN)make health-api$(NC)        - API (8080)\n"
	@printf "  $(CYAN)make health-web$(NC)        - Web (3000)\n"
	@printf "\n"
	@printf "Dica: Use HEALTH_SERVICE para scripts\n"
	@printf "  $(CYAN)make health HEALTH_SERVICE=redis$(NC)\n"

health-all:  ## Health check de todos os serviços
	@./scripts/health-check.sh all

health-postgres:  ## Verifica saúde do PostgreSQL
	@./scripts/health-check.sh postgres

health-redis:  ## Verifica saúde do Redis
	@./scripts/health-check.sh redis

health-market-data:  ## Verifica saúde do Market Data
	@./scripts/health-check.sh market-data

health-analytics:  ## Verifica saúde do Analytics
	@./scripts/health-check.sh analytics

health-strategy:  ## Verifica saúde do Strategy
	@./scripts/health-check.sh strategy

health-executor:  ## Verifica saúde do Executor
	@./scripts/health-check.sh executor

health-monitor:  ## Verifica saúde do Monitor
	@./scripts/health-check.sh monitor

health-backtest:  ## Verifica saúde do Backtest
	@./scripts/health-check.sh backtest

health-api:  ## Verifica saúde do API
	@./scripts/health-check.sh api

health-web:  ## Verifica saúde do Web
	@./scripts/health-check.sh web

# ═══════════════════════════════════════════════════════════════════════════
# VALIDATE RUNTIME
# ═══════════════════════════════════════════════════════════════════════════

.PHONY: validate-runtime validate-runtime-start validate-runtime-check validate-runtime-subscribers validate-runtime-events

validate-runtime:  ##@ Validate Validação completa do runtime (bridge mode)
	@./scripts/validate-runtime.sh bridge all

validate-runtime-start:  ## Validate Iniciar stack
	@./scripts/validate-runtime.sh bridge start

validate-runtime-check:  ## Validate Health check
	@./scripts/validate-runtime.sh bridge check

validate-runtime-subscribers:  ## Validate Redis subscribers
	@./scripts/validate-runtime.sh bridge subscribers

validate-runtime-events:  ## Validate Event flow
	@./scripts/validate-runtime.sh bridge events

# ═══════════════════════════════════════════════════════════════════════════
# DATA MANAGEMENT (Redis + PostgreSQL)
# ═══════════════════════════════════════════════════════════════════════════

.PHONY: data data-status data-health data-restart data-logs data-backup data-postgres data-redis

data:  ##@ Data Data management menu (Redis + PostgreSQL)
	@./scripts/data.sh help

data-status:  ## Data Mostrar status de Redis e PostgreSQL
	@./scripts/data.sh all status

data-health:  ## Data Health check de Redis e PostgreSQL
	@./scripts/data.sh all health

data-restart:  ## Data Reiniciar Redis e PostgreSQL
	@./scripts/data.sh all restart

data-logs:  ## Data Mostrar logs de Redis e PostgreSQL
	@./scripts/data.sh all logs

data-backup:  ## Data Criar backup de Redis e PostgreSQL
	@./scripts/data.sh all backup

data-postgres:  ## Data PostgreSQL management (use: make data-postgres ACTION=status)
	@printf "$(YELLOW)PostgreSQL commands:${NC}\n"
	@printf "  make data-postgres-status   - Status\n"
	@printf "  make data-postgres-health   - Health check\n"
	@printf "  make data-postgres-restart  - Restart\n"
	@printf "  make data-postgres-logs     - Logs\n"
	@printf "  make data-postgres-shell    - psql shell\n"
	@printf "  make data-postgres-backup   - Backup\n"

data-postgres-status:  ## PostgreSQL status
	@./scripts/data.sh postgres status

data-postgres-health:  ## PostgreSQL health check
	@./scripts/data.sh postgres health

data-postgres-restart:  ## PostgreSQL restart
	@./scripts/data.sh postgres restart

data-postgres-logs:  ## PostgreSQL logs
	@./scripts/data.sh postgres logs

data-postgres-shell:  ## PostgreSQL psql shell
	@./scripts/data.sh postgres shell

data-postgres-backup:  ## PostgreSQL backup
	@./scripts/data.sh postgres backup

data-redis:  ## Data Redis management (use: make data-redis ACTION=status)
	@printf "$(YELLOW)Redis commands:${NC}\n"
	@printf "  make data-redis-status   - Status\n"
	@printf "  make data-redis-health   - Health check\n"
	@printf "  make data-redis-restart  - Restart\n"
	@printf "  make data-redis-logs     - Logs\n"
	@printf "  make data-redis-shell    - redis-cli\n"
	@printf "  make data-redis-backup   - RDB snapshot\n"

data-redis-status:  ## Redis status
	@./scripts/data.sh redis status

data-redis-health:  ## Redis health check
	@./scripts/data.sh redis health

data-redis-restart:  ## Redis restart
	@./scripts/data.sh redis restart

data-redis-logs:  ## Redis logs
	@./scripts/data.sh redis logs

data-redis-shell:  ## Redis CLI
	@./scripts/data.sh redis shell

data-redis-backup:  ## Redis RDB snapshot
	@./scripts/data.sh redis backup

# ═══════════════════════════════════════════════════════════════════════════
# UTILITÁRIOS
# ═══════════════════════════════════════════════════════════════════════════

.PHONY: version

version:  ##@ Mostra versões das ferramentas
	@printf "$(YELLOW)→$(NC) Versões:\n"
	@printf "  $(CYAN)Rust:$(NC) $$($(CARGO) --version)\n"
	@printf "  $(CYAN)Node:$(NC) $$($(YARN) --version 2>/dev/null || echo 'não instalado')\n"
	@printf "  $(CYAN)Docker:$(NC) $$($(DOCKER) --version 2>/dev/null || echo 'não instalado')\n"
	@printf "  $(CYAN)Docker Compose:$(NC) $$($(DOCKER) compose version 2>/dev/null || echo 'não instalado')\n"

# ═══════════════════════════════════════════════════════════════════════════
# FIM DO MAKEFILE
# ═══════════════════════════════════════════════════════════════════════════
