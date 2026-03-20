#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
SCRIPT_DIR="$ROOT_DIR/scripts"
cd "$ROOT_DIR"

. "$SCRIPT_DIR/lib/common.sh"
. "$SCRIPT_DIR/container-runtime.sh"

MODE="${1:-bridge}"
ACTION="${2:-all}"
COMPOSE_SCRIPT="./scripts/compose.sh"

PASS_COUNT=0
FAIL_COUNT=0
WARN_COUNT=0

print_header() {
  vt_print_header "ViperTrade - Runtime Validation"
}

print_step() {
  vt_step "$1"
}

print_ok() {
  PASS_COUNT=$((PASS_COUNT + 1))
  vt_ok "$1"
}

print_warn() {
  WARN_COUNT=$((WARN_COUNT + 1))
  vt_warn "$1"
}

print_fail() {
  FAIL_COUNT=$((FAIL_COUNT + 1))
  vt_fail "$1"
}

run_check() {
  local label="$1"
  shift
  print_step "$label"
  if "$@"; then
    print_ok "$label"
  else
    print_fail "$label"
  fi
}

show_help() {
  print_header
  echo ""
  echo "Usage: $0 [bridge] [start|health|subscribers|events|all]"
  echo ""
  echo "Examples:"
  echo "  $0 bridge all"
  echo "  $0 bridge health"
}

ensure_compose_script() {
  [[ -x "$COMPOSE_SCRIPT" ]]
}

start_stack() {
  "$COMPOSE_SCRIPT" up -d
}

check_health() {
  ./scripts/health-check.sh all
}

check_subscribers() {
  local output
  output=$(container_exec vipertrade-redis redis-cli PUBSUB NUMSUB viper:market-signals viper:decisions 2>/dev/null || true)
  [[ -n "$output" ]]
}

check_events() {
  local strategy_logs executor_logs
  strategy_logs=$(container_logs --tail 100 vipertrade-strategy 2>&1 || true)
  executor_logs=$(container_logs --tail 100 vipertrade-executor 2>&1 || true)

  grep -Eq 'Published decision event|action=' <<< "$strategy_logs$executor_logs"
}

print_summary() {
  echo ""
  echo -e "${VT_CYAN}Summary:${VT_NC} PASS=$PASS_COUNT WARN=$WARN_COUNT FAIL=$FAIL_COUNT"
}

main() {
  if [[ "$MODE" == "help" || "$MODE" == "-h" || "$MODE" == "--help" ]]; then
    show_help
    exit 0
  fi

  if [[ "$MODE" != "bridge" ]]; then
    print_fail "Unrecognized mode '$MODE'"
    show_help
    exit 1
  fi

  print_header
  run_check "Compose script available" ensure_compose_script

  case "$ACTION" in
    start)
      run_check "Start stack ($MODE)" start_stack
      ;;
    health)
      run_check "Health checks" check_health
      ;;
    subscribers)
      run_check "Redis subscribers" check_subscribers
      ;;
    events)
      run_check "Recent event flow" check_events
      ;;
    all)
      run_check "Start stack ($MODE)" start_stack
      run_check "Health checks" check_health
      run_check "Redis subscribers" check_subscribers
      run_check "Recent event flow" check_events
      ;;
    *)
      print_fail "Unrecognized action '$ACTION'"
      show_help
      exit 1
      ;;
  esac

  print_summary
  [[ "$FAIL_COUNT" -eq 0 ]]
}

main "$@"
