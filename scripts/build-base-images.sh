#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
BASE_DIR="$ROOT_DIR/docker/base"

RUST_VERSION="${RUST_VERSION:-1.83}"
NODE_VERSION="${NODE_VERSION:-20}"

RUST_BUILDER_IMAGE="${RUST_BUILDER_IMAGE:-vipertrade-base-rust-builder:${RUST_VERSION}}"
RUST_RUNTIME_IMAGE="${RUST_RUNTIME_IMAGE:-vipertrade-base-rust-runtime:bookworm}"
STRATEGY_BUILDER_IMAGE="${STRATEGY_BUILDER_IMAGE:-vipertrade-base-strategy-builder:${RUST_VERSION}}"
STRATEGY_RUNTIME_IMAGE="${STRATEGY_RUNTIME_IMAGE:-vipertrade-base-strategy-runtime:3.12-bookworm}"
WEB_BASE_IMAGE="${WEB_BASE_IMAGE:-vipertrade-base-web-node:${NODE_VERSION}-bookworm}"

if [[ "${CONTAINER_ENGINE:-}" == "docker" ]]; then
  ENGINE="docker"
elif [[ "${CONTAINER_ENGINE:-}" == "podman" ]]; then
  ENGINE="podman"
elif command -v docker >/dev/null 2>&1; then
  ENGINE="docker"
elif command -v podman >/dev/null 2>&1; then
  ENGINE="podman"
else
  echo "ERROR: docker or podman not found" >&2
  exit 1
fi

echo "Building base images..."
echo "  ENGINE=$ENGINE"
echo "  RUST_BUILDER_IMAGE=$RUST_BUILDER_IMAGE"
echo "  RUST_RUNTIME_IMAGE=$RUST_RUNTIME_IMAGE"
echo "  STRATEGY_BUILDER_IMAGE=$STRATEGY_BUILDER_IMAGE"
echo "  STRATEGY_RUNTIME_IMAGE=$STRATEGY_RUNTIME_IMAGE"
echo "  WEB_BASE_IMAGE=$WEB_BASE_IMAGE"

"$ENGINE" build -f "$BASE_DIR/rust-builder.Dockerfile" --build-arg RUST_VERSION="$RUST_VERSION" -t "$RUST_BUILDER_IMAGE" "$ROOT_DIR"
"$ENGINE" build -f "$BASE_DIR/rust-runtime.Dockerfile" -t "$RUST_RUNTIME_IMAGE" "$ROOT_DIR"
"$ENGINE" build -f "$BASE_DIR/strategy-builder.Dockerfile" --build-arg RUST_VERSION="$RUST_VERSION" -t "$STRATEGY_BUILDER_IMAGE" "$ROOT_DIR"
"$ENGINE" build -f "$BASE_DIR/strategy-runtime.Dockerfile" -t "$STRATEGY_RUNTIME_IMAGE" "$ROOT_DIR"
"$ENGINE" build -f "$BASE_DIR/web-node.Dockerfile" --build-arg NODE_VERSION="$NODE_VERSION" -t "$WEB_BASE_IMAGE" "$ROOT_DIR"

echo "Done."
