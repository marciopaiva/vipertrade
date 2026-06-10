SHELL := /bin/bash

# ViperTrade — Kind cluster lifecycle: build → deploy → start → stop.
# Other tooling (validation, health, kill-switch, data reset, compose) lives
# under scripts/ and is invoked directly.

KIND_CLUSTER     ?= dev
KIND_NODE        := $(KIND_CLUSTER)-control-plane
REGISTRY_CTR     ?= kind-registry
CONTAINER_ENGINE ?= $(shell command -v podman >/dev/null 2>&1 && echo podman || echo docker)

KUBE_NAMESPACE   ?= vipertrade

.DEFAULT_GOAL := help
.PHONY: help build build-web deploy start stop

## Show the lifecycle targets
help:
	@printf "ViperTrade — Kind lifecycle\n\n"
	@printf "  make build      Build all service images and push to the local registry\n"
	@printf "  make build-web  Rebuild ONLY the web image and restart its rollout (fast UI iteration)\n"
	@printf "  make deploy     Apply the Kubernetes manifests to the Kind cluster\n"
	@printf "  make start      Start the Kind cluster and local registry\n"
	@printf "  make stop       Stop the Kind cluster and local registry\n"

## Build all service images (web first) and push them to the local Kind registry
build:
	@cd services/web && yarn build
	@./scripts/kind/build-images.sh

## Rebuild only the web image and roll the web deployment so the new image is
## pulled (the :dev tag is mutable, so a restart is required). Avoids the slow
## Rust rebuild that `make build` triggers on any repo change.
build-web:
	@cd services/web && yarn build
	@./scripts/kind/build-images.sh web
	@kubectl rollout restart deployment web -n $(KUBE_NAMESPACE)
	@kubectl rollout status deployment web -n $(KUBE_NAMESPACE) --timeout=120s

## Apply the Kubernetes manifests to the Kind cluster
deploy:
	@./scripts/kind/deploy.sh

## Start the Kind cluster and local registry
start:
	@$(CONTAINER_ENGINE) start $(REGISTRY_CTR) $(KIND_NODE)

## Stop the Kind cluster and local registry
stop:
	@$(CONTAINER_ENGINE) stop $(KIND_NODE) $(REGISTRY_CTR)
