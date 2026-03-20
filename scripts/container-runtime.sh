#!/usr/bin/env bash

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$SCRIPT_DIR/lib/common.sh"

COMPOSE_FILE="${COMPOSE_FILE:-$(vt_root_dir)/compose/docker-compose.yml}"

require_docker() {
  vt_require_cmd docker || exit 1
}

require_docker_compose() {
  require_docker
  if ! docker compose version >/dev/null 2>&1; then
    vt_fail "docker compose not found"
    exit 1
  fi
}

container_exec() {
  require_docker
  docker exec "$@"
}

container_exec_i() {
  require_docker
  docker exec -i "$@"
}

container_logs() {
  require_docker
  docker logs "$@"
}

compose_exec() {
  require_docker_compose
  docker compose -f "$COMPOSE_FILE" exec "$@"
}

compose_exec_t() {
  require_docker_compose
  docker compose -f "$COMPOSE_FILE" exec -T "$@"
}
