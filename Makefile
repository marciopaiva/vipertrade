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
	@printf "  $(CYAN)make health-all$(NC)       - Todos os serviços\n"
	@printf "  $(CYAN)make health-postgres$(NC)  - PostgreSQL\n"
	@printf "  $(CYAN)make health-redis$(NC)     - Redis\n"
	@printf "  $(CYAN)make health-strategy$(NC)  - Strategy Service\n"
	@printf "  $(CYAN)make health-executor$(NC)  - Executor Service\n"
	@printf "  $(CYAN)make health-api$(NC)       - API Service\n"
	@printf "  $(CYAN)make health-web$(NC)       - Web Dashboard\n"
	@printf "\n"
	@printf "Dica: Use HEALTH_SERVICE para scripts\n"
	@printf "  $(CYAN)make health HEALTH_SERVICE=redis$(NC)\n"

health-all:  ## Health check de todos os serviços
	@printf "$(YELLOW)→$(NC) Health Checks - Todos os serviços...\n\n"
	@$(MAKE) -s health-postgres
	@$(MAKE) -s health-redis
	@$(MAKE) -s health-strategy
	@$(MAKE) -s health-executor
	@$(MAKE) -s health-api
	@$(MAKE) -s health-web
	@printf "\n$(GREEN)✓$(NC) Health checks completos\n"

health-postgres:  ## Verifica saúde do PostgreSQL
	@printf "$(YELLOW)→$(NC) Health: PostgreSQL...\n"
	@$(DOCKER) exec vipertrade-postgres pg_isready -U $(DB_USER) -d $(DB_NAME) > /dev/null 2>&1 && \
		printf "$(GREEN)✓$(NC) PostgreSQL OK\n" || \
		printf "$(RED)✗$(NC) PostgreSQL não disponível\n"

health-redis:  ## Verifica saúde do Redis
	@printf "$(YELLOW)→$(NC) Health: Redis...\n"
	@$(DOCKER) exec vipertrade-redis redis-cli ping && \
		printf "$(GREEN)✓$(NC) Redis OK\n" || \
		printf "$(RED)✗$(NC) Redis não disponível\n"

health-strategy:  ## Verifica saúde do serviço strategy
	@printf "$(YELLOW)→$(NC) Health: Strategy...\n"
	@curl -sf http://localhost:8082/health > /dev/null 2>&1 && \
		printf "$(GREEN)✓$(NC) Strategy OK\n" || \
		printf "$(RED)✗$(NC) Strategy não disponível\n"

health-executor:  ## Verifica saúde do serviço executor
	@printf "$(YELLOW)→$(NC) Health: Executor...\n"
	@curl -sf http://localhost:8083/health > /dev/null 2>&1 && \
		printf "$(GREEN)✓$(NC) Executor OK\n" || \
		printf "$(RED)✗$(NC) Executor não disponível\n"

health-api:  ## Verifica saúde do serviço api
	@printf "$(YELLOW)→$(NC) Health: API...\n"
	@curl -sf http://localhost:8080/health > /dev/null 2>&1 && \
		printf "$(GREEN)✓$(NC) API OK\n" || \
		printf "$(RED)✗$(NC) API não disponível\n"

health-web:  ## Verifica saúde do serviço web
	@printf "$(YELLOW)→$(NC) Health: Web...\n"
	@curl -sf http://localhost:3000 > /dev/null 2>&1 && \
		printf "$(GREEN)✓$(NC) Web OK\n" || \
		printf "$(RED)✗$(NC) Web não disponível\n"

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
