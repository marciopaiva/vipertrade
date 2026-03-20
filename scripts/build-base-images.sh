#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$SCRIPT_DIR/lib/common.sh"

ROOT_DIR="$(vt_root_dir)"
BASE_DIR="$ROOT_DIR/docker/base"

RUST_VERSION="${RUST_VERSION:-1.83}"
NODE_VERSION="${NODE_VERSION:-20}"

RUST_BUILDER_IMAGE="${RUST_BUILDER_IMAGE:-vipertrade-base-rust-builder:${RUST_VERSION}}"
RUST_RUNTIME_IMAGE="${RUST_RUNTIME_IMAGE:-vipertrade-base-rust-runtime:bookworm}"
STRATEGY_BUILDER_IMAGE="${STRATEGY_BUILDER_IMAGE:-vipertrade-base-strategy-builder:${RUST_VERSION}}"
STRATEGY_RUNTIME_IMAGE="${STRATEGY_RUNTIME_IMAGE:-vipertrade-base-strategy-runtime:3.12-bookworm}"
WEB_BASE_IMAGE="${WEB_BASE_IMAGE:-vipertrade-base-web-node:${NODE_VERSION}-bookworm}"

show_help() {
  vt_print_header "ViperTrade - Build Base Images"
  echo ""
  echo "Usage:"
  echo "  ./scripts/build-base-images.sh"
  echo ""
  echo "Examples:"
  echo "  ./scripts/build-base-images.sh"
  echo "  RUST_VERSION=1.83 NODE_VERSION=20 ./scripts/build-base-images.sh"
  echo ""
  echo "Environment:"
  echo "  RUST_VERSION            Rust toolchain version (default: 1.83)"
  echo "  NODE_VERSION            Node.js version (default: 20)"
  echo "  RUST_BUILDER_IMAGE      Rust builder image tag"
  echo "  RUST_RUNTIME_IMAGE      Rust runtime image tag"
  echo "  STRATEGY_BUILDER_IMAGE  Strategy builder image tag"
  echo "  STRATEGY_RUNTIME_IMAGE  Strategy runtime image tag"
  echo "  WEB_BASE_IMAGE          Web base image tag"
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" || "${1:-}" == "help" ]]; then
  show_help
  exit 0
fi

vt_require_cmd docker

vt_print_header "ViperTrade - Build Base Images"
vt_info "ENGINE=docker"
vt_info "RUST_BUILDER_IMAGE=$RUST_BUILDER_IMAGE"
vt_info "RUST_RUNTIME_IMAGE=$RUST_RUNTIME_IMAGE"
vt_info "STRATEGY_BUILDER_IMAGE=$STRATEGY_BUILDER_IMAGE"
vt_info "STRATEGY_RUNTIME_IMAGE=$STRATEGY_RUNTIME_IMAGE"
vt_info "WEB_BASE_IMAGE=$WEB_BASE_IMAGE"

vt_step "Building rust builder"
docker build -f "$BASE_DIR/rust-builder.Dockerfile" --build-arg RUST_VERSION="$RUST_VERSION" -t "$RUST_BUILDER_IMAGE" "$ROOT_DIR"
vt_step "Building rust runtime"
docker build -f "$BASE_DIR/rust-runtime.Dockerfile" -t "$RUST_RUNTIME_IMAGE" "$ROOT_DIR"
vt_step "Building strategy builder"
docker build -f "$BASE_DIR/strategy-builder.Dockerfile" --build-arg RUST_VERSION="$RUST_VERSION" -t "$STRATEGY_BUILDER_IMAGE" "$ROOT_DIR"
vt_step "Building strategy runtime"
docker build -f "$BASE_DIR/strategy-runtime.Dockerfile" -t "$STRATEGY_RUNTIME_IMAGE" "$ROOT_DIR"
vt_step "Building web base image"
docker build -f "$BASE_DIR/web-node.Dockerfile" --build-arg NODE_VERSION="$NODE_VERSION" -t "$WEB_BASE_IMAGE" "$ROOT_DIR"

vt_ok "Base images build complete"
