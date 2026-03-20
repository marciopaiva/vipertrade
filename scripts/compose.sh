#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$SCRIPT_DIR/lib/common.sh"

ROOT_DIR="$(vt_root_dir)"
COMPOSE_DIR="$ROOT_DIR/compose"
COMPOSE_FILE="$COMPOSE_DIR/docker-compose.yml"
COMPOSE_ENV_FILE="$COMPOSE_DIR/.env"
DOWN_TIMEOUT="${COMPOSE_DOWN_TIMEOUT:-20}"

if [[ ! -f "$COMPOSE_ENV_FILE" ]]; then
  COMPOSE_ENV_FILE="$COMPOSE_DIR/.env.example"
fi

print_header() {
  vt_print_header "ViperTrade - Compose Bridge"
}

print_step() {
  vt_step "$1"
}

print_ok() {
  vt_ok "$1"
}

print_fail() {
  vt_fail "$1"
}

show_help() {
  print_header
  echo ""
  echo "Usage: $0 [compose command]"
  echo ""
  echo "Examples:"
  echo "  $0 up -d --build"
  echo "  $0 ps"
  echo "  $0 logs strategy"
  echo "  $0 config"
  echo "  $0 down"
  echo ""
  echo "Runtime:"
  echo "  docker compose"
}

run_compose() {
  if ! docker compose version >/dev/null 2>&1; then
    print_fail "docker compose not found"
    exit 1
  fi

  docker compose --env-file "$COMPOSE_ENV_FILE" -f "$COMPOSE_FILE" "$@"
}

run_down_tolerant() {
  local tmp rc
  tmp=$(mktemp)
  set +e
  run_compose down --timeout "$DOWN_TIMEOUT" "${@:2}" > >(cat) 2>"$tmp"
  rc=$?
  set -e

  grep -Ev 'no such container|no pod with name or ID' "$tmp" >&2 || true
  rm -f "$tmp"
  return $rc
}

main() {
  local command="${1:-help}"

  if [[ "$command" == "help" || "$command" == "-h" || "$command" == "--help" ]]; then
    show_help
    exit 0
  fi

  cd "$COMPOSE_DIR"

  print_step "Provider: docker compose"
  print_step "Compose file: $COMPOSE_FILE"

  if [[ "$command" == "down" ]]; then
    if ! run_down_tolerant "$@"; then
      print_fail "compose down failed"
      exit 1
    fi
    print_ok "Compose down completed"
    exit 0
  fi

  run_compose "$@"
}

main "$@"
