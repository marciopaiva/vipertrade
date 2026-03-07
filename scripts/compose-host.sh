#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
COMPOSE_DIR="$ROOT_DIR/compose"
HOST_FILE="$COMPOSE_DIR/docker-compose.host.yml"
COMPOSE_ENV_FILE="$COMPOSE_DIR/.env"

if [[ ! -f "$COMPOSE_ENV_FILE" ]]; then
  COMPOSE_ENV_FILE="$COMPOSE_DIR/.env.example"
fi

if [[ ! -f "$HOST_FILE" ]]; then
  echo "ERROR: $HOST_FILE not found" >&2
  exit 1
fi

if [[ "${COMPOSE_PROVIDER:-}" == "podman-compose" ]]; then
  PROVIDER="podman-compose"
elif [[ "${COMPOSE_PROVIDER:-}" == "podman-compose-plugin" ]]; then
  PROVIDER="podman-compose-plugin"
elif command -v podman-compose >/dev/null 2>&1; then
  PROVIDER="podman-compose"
elif podman compose version >/dev/null 2>&1; then
  PROVIDER="podman-compose-plugin"
else
  echo "ERROR: podman-compose or podman compose not found" >&2
  exit 1
fi

cd "$COMPOSE_DIR"

if [[ "$PROVIDER" == "podman-compose" ]]; then
  exec podman-compose -f "$HOST_FILE" "$@"
else
  exec podman compose --env-file "$COMPOSE_ENV_FILE" -f "$HOST_FILE" "$@"
fi
