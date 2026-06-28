SHELL := /bin/bash

# ViperTrade — Kind cluster lifecycle: build → deploy → start → stop.
# Other tooling (validation, health, kill-switch, data reset, compose) lives
# under scripts/ and is invoked directly.

KIND_CLUSTER     ?= dev
KIND_NODE        := $(KIND_CLUSTER)-control-plane
REGISTRY_CTR     ?= kind-registry
CONTAINER_ENGINE ?= $(shell command -v podman >/dev/null 2>&1 && echo podman || echo docker)

KUBE_NAMESPACE   ?= vipertrade
KIND_CONTEXT     ?= kind-$(KIND_CLUSTER)

# App deployments (stateful postgres/redis excluded) — restarted on redeploy so
# they pick up the mutable :dev image and re-read ConfigMap env.
APP_DEPLOYMENTS  := market-data analytics strategy executor monitor api ai-analyst web

.DEFAULT_GOAL := help
.PHONY: help build build-web deploy redeploy start stop wipe

## Show the lifecycle targets
help:
	@printf "ViperTrade — Kind lifecycle\n\n"
	@printf "  make build      Build all service images and push to the local registry\n"
	@printf "  make build-web  Rebuild ONLY the web image (Docker-based, no host tooling needed)\n"
	@printf "  make deploy     Apply the Kubernetes manifests to the Kind cluster\n"
	@printf "  make redeploy   Build + deploy + restart app pods (pick up new image & config)\n"
	@printf "  make start      Start the Kind cluster and local registry\n"
	@printf "  make stop       Stop the Kind cluster and local registry\n"
	@printf "  make wipe       Wipe paper trading data from postgres + restart services (fresh start)\n"

## Build all service images and push them to the local Kind registry
build:
	@NEXT_PUBLIC_WS_URL=ws://localhost:8443/ws ./scripts/kind/build-images.sh

## Rebuild only the web image and roll the web deployment so the new image is
## pulled (the :dev tag is mutable, so a restart is required). Avoids the slow
## Rust rebuild that `make build` triggers on any repo change.
build-web:
	@NEXT_PUBLIC_WS_URL=ws://localhost:8443/ws ./scripts/kind/build-images.sh web
	@kubectl --context $(KIND_CONTEXT) rollout restart deployment web -n $(KUBE_NAMESPACE)
	@kubectl --context $(KIND_CONTEXT) rollout status deployment web -n $(KUBE_NAMESPACE) --timeout=120s

## Apply the Kubernetes manifests to the Kind cluster
deploy:
	@./scripts/kind/deploy.sh

## Full refresh after a code or config change: rebuild images, apply manifests,
## then restart every app deployment so they pick up the new (mutable :dev) image
## and ConfigMap env. Does its own apply + rollout restart instead of depending on
## `deploy`: an image-only change leaves the Deployment spec "unchanged" (apply
## won't recreate pods), and `deploy`'s rollout-status wait is fatal if a prior
## rollout is stuck — either would skip the restart. `restart` forces a fresh
## ReplicaSet, which also clears a stuck rollout.
redeploy: build
	@kubectl --context $(KIND_CONTEXT) apply -k k8s/kind
	@kubectl --context $(KIND_CONTEXT) -n $(KUBE_NAMESPACE) rollout restart deployment $(APP_DEPLOYMENTS)
	@for d in $(APP_DEPLOYMENTS); do \
		kubectl --context $(KIND_CONTEXT) -n $(KUBE_NAMESPACE) rollout status deployment $$d --timeout=300s & \
	done; \
	status=0; \
	for job in $$(jobs -p); do \
		wait $$job || status=1; \
	done; \
	[ $$status -eq 0 ] || (echo "One or more rollouts failed" >&2 && exit 1)

## Start the Kind cluster and local registry
start:
	@$(CONTAINER_ENGINE) start $(REGISTRY_CTR) $(KIND_NODE)

## Stop the Kind cluster and local registry
stop:
	@$(CONTAINER_ENGINE) stop $(KIND_NODE) $(REGISTRY_CTR)

## Wipe paper trading data from the cluster's postgres and restart the stateful
## services so they come back clean. Destructive — prompts for confirmation
## (skip with `make wipe CONFIRM=yes`). Config is unaffected (lives in pairs.yaml).
wipe:
	@./scripts/kind/wipe.sh
