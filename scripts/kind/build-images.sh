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

# Unified multi-role Rust binary: one image (vipertrade:TAG) for all 7 services,
# selected at runtime via VIPER_ROLE. Uses the strategy base images (python +
# cargo-tupa runtime) so every role is covered.
vt_step "Building $KIND_REGISTRY/vipertrade:$IMAGE_TAG (unified viper)"
vt_container build -t "$KIND_REGISTRY/vipertrade:$IMAGE_TAG" -f services/viper/Dockerfile \
  --build-arg TUPA_VERSION="${TUPA_VERSION:-v0.10.0}" \
  --build-arg BUILDER_IMAGE="${STRATEGY_BUILDER_IMAGE:-vipertrade-base-strategy-builder:1.83}" \
  --build-arg RUNTIME_IMAGE="${STRATEGY_RUNTIME_IMAGE:-vipertrade-base-strategy-runtime:3.12-bookworm}" \
  .
vt_step "Pushing $KIND_REGISTRY/vipertrade:$IMAGE_TAG"
vt_container push "$KIND_REGISTRY/vipertrade:$IMAGE_TAG"

# NEXT_PUBLIC_API_URL is intentionally empty: client fetches use relative
# /api/... paths proxied by the Next rewrite (NEXT_REWRITE_API_URL -> api:8080).
# An absolute base would break in the browser (can't resolve `api:8080`) / CORS.
build_image web services/web services/web/Dockerfile --build-arg NODE_VERSION=20 --build-arg NEXT_PUBLIC_API_URL="${NEXT_PUBLIC_API_URL:-}" --build-arg NEXT_PUBLIC_TRADING_MODE="${NEXT_PUBLIC_TRADING_MODE:-paper}"

vt_ok "All images built and pushed to $KIND_REGISTRY"


