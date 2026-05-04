#!/usr/bin/env bash

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$SCRIPT_DIR/lib/common.sh"

COMPOSE_FILE="${COMPOSE_FILE:-$(vt_root_dir)/compose/docker-compose.yml}"

require_container() {
  vt_container_available || exit 1
}

require_compose() {
  if ! vt_compose_available; then
    vt_fail "compose runtime not found"
    exit 1
  fi
}

container_exec() {
  require_container
  vt_container exec "$@"
}

container_exec_i() {
  require_container
  vt_container exec -i "$@"
}

container_logs() {
  require_container
  vt_container logs "$@"
}

compose_exec() {
  require_compose
  vt_compose -f "$COMPOSE_FILE" exec "$@"
}

compose_exec_t() {
  require_compose
  vt_compose -f "$COMPOSE_FILE" exec -T "$@"
}
