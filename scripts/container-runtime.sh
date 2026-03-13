#!/usr/bin/env bash
set -euo pipefail

detect_container_engine() {
  if [[ "${CONTAINER_ENGINE:-}" == "docker" ]]; then
    echo "docker"
  elif [[ "${CONTAINER_ENGINE:-}" == "podman" ]]; then
    echo "podman"
  elif command -v docker >/dev/null 2>&1; then
    echo "docker"
  elif command -v podman >/dev/null 2>&1; then
    echo "podman"
  else
    echo "ERROR: docker or podman not found" >&2
    return 1
  fi
}

container_exec() {
  local engine
  engine="$(detect_container_engine)"
  "$engine" exec "$@"
}

container_exec_i() {
  local engine
  engine="$(detect_container_engine)"
  "$engine" exec -i "$@"
}

container_logs() {
  local engine
  engine="$(detect_container_engine)"
  "$engine" logs "$@"
}

container_inspect() {
  local engine
  engine="$(detect_container_engine)"
  "$engine" inspect "$@"
}
