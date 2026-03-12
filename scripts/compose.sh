#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
COMPOSE_DIR="$ROOT_DIR/compose"
COMPOSE_FILE="$COMPOSE_DIR/docker-compose.yml"
COMPOSE_ENV_FILE="$COMPOSE_DIR/.env"
DOWN_TIMEOUT="${COMPOSE_DOWN_TIMEOUT:-20}"

if [[ ! -f "$COMPOSE_ENV_FILE" ]]; then
  COMPOSE_ENV_FILE="$COMPOSE_DIR/.env.example"
fi

if [[ "${COMPOSE_PROVIDER:-}" == "docker-compose-plugin" ]]; then
  PROVIDER="docker-compose-plugin"
elif [[ "${COMPOSE_PROVIDER:-}" == "podman-compose" ]]; then
  PROVIDER="podman-compose"
elif [[ "${COMPOSE_PROVIDER:-}" == "podman-compose-plugin" ]]; then
  PROVIDER="podman-compose-plugin"
elif docker compose version >/dev/null 2>&1; then
  PROVIDER="docker-compose-plugin"
elif command -v podman-compose >/dev/null 2>&1; then
  PROVIDER="podman-compose"
elif podman compose version >/dev/null 2>&1; then
  PROVIDER="podman-compose-plugin"
else
  echo "ERROR: docker compose, podman-compose, or podman compose not found" >&2
  exit 1
fi

run_compose() {
  if [[ "$PROVIDER" == "docker-compose-plugin" ]]; then
    docker compose --env-file "$COMPOSE_ENV_FILE" -f "$COMPOSE_FILE" "$@"
  elif [[ "$PROVIDER" == "podman-compose" ]]; then
    podman-compose -f "$COMPOSE_FILE" "$@"
  else
    podman compose --env-file "$COMPOSE_ENV_FILE" -f "$COMPOSE_FILE" "$@"
  fi
}

force_cleanup_viper() {
  if [[ "$PROVIDER" == "docker-compose-plugin" ]]; then
    return 0
  fi

  local names
  names=$(podman ps -a --format '{{.Names}}' | grep '^vipertrade-' || true)
  if [[ -n "$names" ]]; then
    echo "WARN: forcing cleanup for lingering ViperTrade containers" >&2
    while IFS= read -r name; do
      [[ -z "$name" ]] && continue
      podman stop -t 2 "$name" >/dev/null 2>&1 || true
      podman rm -f "$name" >/dev/null 2>&1 || true
    done <<< "$names"
  fi

  podman network rm compose_viper-net >/dev/null 2>&1 || true
}

run_down_tolerant() {
  local tmp rc
  tmp=$(mktemp)
  set +e
  run_compose down --timeout "$DOWN_TIMEOUT" "${@:2}" > >(cat) 2>"$tmp"
  rc=$?
  set -e

  # Suppress noisy, benign not-found errors common in WSL+Podman down cleanup.
  grep -Ev 'no such container|no pod with name or ID' "$tmp" >&2 || true
  rm -f "$tmp"

  return $rc
}

cd "$COMPOSE_DIR"

if [[ "${1:-}" == "down" ]]; then
  if ! run_down_tolerant "$@"; then
    echo "WARN: compose down failed, applying fallback cleanup" >&2
    force_cleanup_viper
    exit 0
  fi

  force_cleanup_viper
  exit 0
fi

run_compose "$@"
