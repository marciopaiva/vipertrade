.DEFAULT_GOAL := help
SHELL := /bin/bash

GREEN  := \033[0;32m
YELLOW := \033[1;33m
BLUE   := \033[0;34m
CYAN   := \033[0;36m
NC     := \033[0m

COMPOSE               ?= ./scripts/compose.sh
HEALTH                ?= ./scripts/health-check.sh
VALIDATE_WORKSPACE    ?= ./scripts/validate-workspace.sh
VALIDATE_RUNTIME      ?= ./scripts/validate-runtime.sh
RESET_PAPER_DB        ?= ./scripts/reset-paper-db.sh
BUILD_BASE_IMAGES     ?= ./scripts/build-base-images.sh
KILL_SWITCH           ?= ./scripts/kill-switch-control.sh

CARGO := cargo
YARN  := yarn
DOCKER := docker

define HEADER

____   ____.__                   ___________                  .___      
\   \ /   /|__|_____   __________\__    ___/___________     __| _/____  
 \   Y   / |  \____ \_/ __ \_  __ \|    |  \_  __ \__  \   / __ |/ __ \ 
  \     /  |  |  |_> >  ___/|  | \/|    |   |  | \// __ \_/ /_/ \  ___/ 
   \___/   |__|   __/ \___  >__|   |____|   |__|  (____  /\____ |\___  >
              |__|        \/                           \/      \/    \/ 

         VIPERTRADE • Lead Trader Bot for Bybit Copy Trading
             TupaLang v0.8.1 • Rust 1.83 • Version 0.8.1
════════════════════════════════════════════════════════════════════════
endef
export HEADER

.PHONY: \
	help \
	health \
	validate validate-full validate-workspace-quick validate-ci validate-runtime \
	install-git-hooks \
	build-base-images \
	compose-up compose-down compose-restart compose-ps compose-logs \
	data-reset-paper-db \
	control-kill-switch-status control-kill-switch-enable control-kill-switch-disable \
	version

## Show all available targets
help:
	@clear 2>/dev/null || true
	@printf "\033c"
	@printf "$$HEADER\n\n"
	@printf "$(YELLOW)ViperTrade Makefile - Task Automation$(NC)\n\n"
	@./scripts/make-help.py
	@printf "\n"
	@printf "Usage: $(GREEN)make$(NC) $(BLUE)[target]$(NC)\n"

## Check the health of all services
health:              ; @$(HEALTH) all

## Run full workspace validation and supporting checks
validate-full:            ; @$(VALIDATE_WORKSPACE) all
## Run fmt, clippy, and tests
validate-workspace-quick: ; @$(VALIDATE_WORKSPACE) quick
## Run the GitHub Actions-equivalent validation before commit/push
validate-ci:              ; @$(VALIDATE_WORKSPACE) ci
## Install the versioned git hooks for local parity before push
install-git-hooks:        ; @./scripts/install-git-hooks.sh
## Validate the bridge runtime end to end
validate-runtime:         ; @$(VALIDATE_RUNTIME) bridge all

## Build the project's base images
build-base-images: ; @$(BUILD_BASE_IMAGES)

## Start the bridge stack with build
compose-up:      ; @$(COMPOSE) up -d --build
## Stop the bridge stack
compose-down:    ; @$(COMPOSE) down
## Restart the bridge stack
compose-restart: ; @$(COMPOSE) down && $(COMPOSE) up -d --build
## List containers in the bridge stack
compose-ps:      ; @$(COMPOSE) ps
## Show logs for the bridge stack
compose-logs:    ; @$(COMPOSE) logs --tail=100

## Reset paper trades and snapshots
data-reset-paper-db: ; @$(RESET_PAPER_DB) --yes

## Show the current kill switch state
control-kill-switch-status:  ; @$(KILL_SWITCH) status
## Enable the global execution block through the API
control-kill-switch-enable:  ; @$(KILL_SWITCH) enable
## Disable the global execution block through the API
control-kill-switch-disable: ; @$(KILL_SWITCH) disable

## Show local tool versions
version:
	@printf "$(YELLOW)→$(NC) Versions:\n"
	@printf "  $(CYAN)Rust:$(NC) $$($(CARGO) --version 2>/dev/null || echo 'not installed')\n"
	@printf "  $(CYAN)Yarn:$(NC) $$($(YARN) --version 2>/dev/null || echo 'not installed')\n"
	@printf "  $(CYAN)Docker:$(NC) $$($(DOCKER) --version 2>/dev/null || echo 'not installed')\n"
	@printf "  $(CYAN)Docker Compose:$(NC) $$($(DOCKER) compose version 2>/dev/null || echo 'not installed')\n"
