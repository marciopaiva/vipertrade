#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../lib/common.sh"

vt_cd_root

# Auto-detect registry for WSL + Podman
: "${KIND_REGISTRY:=$(vt_suggest_registry)}"
IMAGE_TAG="${IMAGE_TAG:-dev}"

vt_print_header "ViperTrade Kind images"
vt_info "ENGINE=$(vt_container_engine)"
vt_info "KIND_REGISTRY=$KIND_REGISTRY"
vt_info "IMAGE_TAG=$IMAGE_TAG"

# Verify registry is accessible
if ! vt_registry_available "$KIND_REGISTRY"; then
  vt_warn "Registry $KIND_REGISTRY is not accessible"
  if vt_is_wsl; then
    vt_info "On WSL, ensure the local registry is running:"
    vt_info "  podman run -d --name kind-registry --network kind -p 127.0.0.1:5001:5000 --restart=always docker.io/library/registry:2"
    vt_info "Or set KIND_REGISTRY=host.docker.internal:5001 if using Docker Desktop on Windows"
  fi
  exit 1
fi

build_image() {
  local name="$1"
  local context="$2"
  local dockerfile="$3"
  shift 3

  local image="$KIND_REGISTRY/vipertrade-$name:$IMAGE_TAG"

  vt_step "Building $image"
  vt_container build -t "$image" -f "$dockerfile" "$@" "$context"

  vt_step "Pushing $image"
  vt_container push "$image"
}

rust_args=(
  --build-arg RUST_VERSION=1.83
  --build-arg RUST_BUILDER_IMAGE="${RUST_BUILDER_IMAGE:-vipertrade-base-rust-builder:1.83}"
  --build-arg RUST_RUNTIME_IMAGE="${RUST_RUNTIME_IMAGE:-vipertrade-base-rust-runtime:bookworm}"
)

build_image postgres . database/Dockerfile
build_image market-data . services/market-data/Dockerfile "${rust_args[@]}"
build_image analytics . services/analytics/Dockerfile "${rust_args[@]}"
build_image strategy . services/strategy/Dockerfile \
  --build-arg TUPA_VERSION="${TUPA_VERSION:-v0.8.1}" \
  --build-arg TUPA_BACKEND="${TUPA_BACKEND:-hybrid}" \
  --build-arg RUST_VERSION=1.83 \
  --build-arg STRATEGY_BUILDER_IMAGE="${STRATEGY_BUILDER_IMAGE:-vipertrade-base-strategy-builder:1.83}" \
  --build-arg STRATEGY_RUNTIME_IMAGE="${STRATEGY_RUNTIME_IMAGE:-vipertrade-base-strategy-runtime:3.12-bookworm}"
build_image executor . services/executor/Dockerfile "${rust_args[@]}"
build_image monitor . services/monitor/Dockerfile "${rust_args[@]}"
build_image backtest . services/backtest/Dockerfile "${rust_args[@]}"
build_image api . services/api/Dockerfile "${rust_args[@]}"
build_image ai-analyst . services/ai-analyst/Dockerfile "${rust_args[@]}"
build_image web services/web services/web/Dockerfile \
  --build-arg NODE_VERSION=20 \
  --build-arg NEXT_PUBLIC_API_URL="${NEXT_PUBLIC_API_URL:-http://api:8080}" \
  --build-arg NEXT_PUBLIC_WS_URL="${NEXT_PUBLIC_WS_URL:-ws://api:8080/ws}" \
  --build-arg NEXT_PUBLIC_TRADING_MODE="${NEXT_PUBLIC_TRADING_MODE:-paper}" \
  --build-arg NEXT_PUBLIC_ENABLE_WEBSOCKET="${NEXT_PUBLIC_ENABLE_WEBSOCKET:-true}" \
  --build-arg NEXT_PUBLIC_ENABLE_ANALYTICS="${NEXT_PUBLIC_ENABLE_ANALYTICS:-true}" \
  --build-arg NEXT_PUBLIC_REFRESH_INTERVAL="${NEXT_PUBLIC_REFRESH_INTERVAL:-5000}"

vt_ok "Images available in $KIND_REGISTRY with tag $IMAGE_TAG"


