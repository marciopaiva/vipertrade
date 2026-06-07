#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/../lib/common.sh"

vt_cd_root

KIND_REGISTRY="${KIND_REGISTRY:-$(vt_suggest_registry)}"
IMAGE_TAG="${IMAGE_TAG:-dev}"

vt_print_header "ViperTrade Kind images"
vt_info "ENGINE=$(vt_container_engine) | REGISTRY=$KIND_REGISTRY | TAG=$IMAGE_TAG"

if ! vt_registry_available "$KIND_REGISTRY"; then
  vt_fail "Registry $KIND_REGISTRY not accessible"
  exit 1
fi

build_image() {
  local name="$1" context="$2" dockerfile="$3"
  shift 3
  vt_step "Building $KIND_REGISTRY/vipertrade-$name:$IMAGE_TAG"
  vt_container build -t "$KIND_REGISTRY/vipertrade-$name:$IMAGE_TAG" -f "$dockerfile" "$@" "$context"
  vt_step "Pushing $KIND_REGISTRY/vipertrade-$name:$IMAGE_TAG"
  vt_container push "$KIND_REGISTRY/vipertrade-$name:$IMAGE_TAG"
}

rust_args=(--build-arg RUST_VERSION=1.83 --build-arg RUST_BUILDER_IMAGE="${RUST_BUILDER_IMAGE:-vipertrade-base-rust-builder:1.83}" --build-arg RUST_RUNTIME_IMAGE="${RUST_RUNTIME_IMAGE:-vipertrade-base-rust-runtime:bookworm}")

build_image postgres . database/Dockerfile
build_image market-data . services/market-data/Dockerfile "${rust_args[@]}"
build_image analytics . services/analytics/Dockerfile "${rust_args[@]}"
build_image strategy . services/strategy/Dockerfile --build-arg TUPA_VERSION="${TUPA_VERSION:-v0.9.5}" --build-arg TUPA_BACKEND="${TUPA_BACKEND:-hybrid}" --build-arg RUST_VERSION=1.83 --build-arg STRATEGY_BUILDER_IMAGE="${STRATEGY_BUILDER_IMAGE:-vipertrade-base-strategy-builder:1.83}" --build-arg STRATEGY_RUNTIME_IMAGE="${STRATEGY_RUNTIME_IMAGE:-vipertrade-base-strategy-runtime:3.12-bookworm}" "${rust_args[@]}"
build_image executor . services/executor/Dockerfile "${rust_args[@]}"
build_image monitor . services/monitor/Dockerfile "${rust_args[@]}"
build_image backtest . services/backtest/Dockerfile "${rust_args[@]}"
build_image api . services/api/Dockerfile "${rust_args[@]}"
build_image ai-analyst . services/ai-analyst/Dockerfile "${rust_args[@]}"
build_image web services/web services/web/Dockerfile --build-arg NODE_VERSION=20 --build-arg NEXT_PUBLIC_API_URL="${NEXT_PUBLIC_API_URL:-http://api:8080}" --build-arg NEXT_PUBLIC_TRADING_MODE="${NEXT_PUBLIC_TRADING_MODE:-paper}"

vt_ok "All images built and pushed to $KIND_REGISTRY"


