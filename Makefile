SHELL := /bin/bash

# ViperTrade — Kind cluster lifecycle: build → deploy → start → stop.
# Other tooling (validation, health, kill-switch, data reset, compose) lives
# under scripts/ and is invoked directly.

KIND_CLUSTER     ?= dev
KIND_NODE        := $(KIND_CLUSTER)-control-plane
REGISTRY_CTR     ?= kind-registry
CONTAINER_ENGINE ?= $(shell command -v podman >/dev/null 2>&1 && echo podman || echo docker)

.DEFAULT_GOAL := help
.PHONY: help build deploy start stop

## Show the lifecycle targets
help:
	@printf "ViperTrade — Kind lifecycle\n\n"
	@printf "  make build   Build all service images and push to the local registry\n"
	@printf "  make deploy  Apply the Kubernetes manifests to the Kind cluster\n"
	@printf "  make start   Start the Kind cluster and local registry\n"
	@printf "  make stop    Stop the Kind cluster and local registry\n"

## Build all service images (web first) and push them to the local Kind registry
build:
	@cd services/web && yarn build
	@./scripts/kind/build-images.sh

## Apply the Kubernetes manifests to the Kind cluster
deploy:
	@./scripts/kind/deploy.sh

## Start the Kind cluster and local registry
start:
	@$(CONTAINER_ENGINE) start $(REGISTRY_CTR) $(KIND_NODE)

## Stop the Kind cluster and local registry
stop:
	@$(CONTAINER_ENGINE) stop $(KIND_NODE) $(REGISTRY_CTR)
