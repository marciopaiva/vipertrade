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

# Kind-specific defaults
KIND_CONTEXT="${KIND_CONTEXT:-kind-dev}"
KIND_NAMESPACE="${KIND_NAMESPACE:-vipertrade}"

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
  echo "Usage: $0 [bridge|kind] [start|health|subscribers|events|all]"
  echo ""
  echo "Examples:"
  echo "  $0 bridge all       # Docker Compose/Podman Compose stack"
  echo "  $0 kind all         # Kubernetes Kind cluster"
  echo "  $0 kind health"
  echo ""
  echo "Modes:"
  echo "  bridge  - local compose stack (docker-compose/podman-compose)"
  echo "  kind    - Kubernetes Kind cluster"
  echo ""
  echo "Actions:"
  echo "  start   - start the stack (bridge only; kind: use make kind-build-images + kind-deploy)"
  echo "  health  - check health/readiness"
  echo "  subs    - verify Redis pub/sub subscribers"
  echo "  events  - check for recent decision/execution events"
  echo "  all     - run all applicable checks for the mode"
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

# ── Kind checks ────────────────────────────────────────────────────────────────

check_kind_cluster() {
  vt_require_cmd kubectl

  local context="${KIND_CONTEXT:-kind-dev}"
  if kubectl cluster-info --context "$context" >/dev/null 2>&1; then
    return 0
  else
    return 1
  fi
}

check_kind_pods_ready() {
  vt_require_cmd kubectl

  local context="${KIND_CONTEXT:-kind-dev}"
  local namespace="${KIND_NAMESPACE:-vipertrade}"

  local not_ready
  not_ready=$(kubectl --context "$context" -n "$namespace" get pods -o jsonpath='{range .items[?(@.status.containerStatuses[0].ready!=true)]}{.metadata.name}{"\n"}{end}' 2>/dev/null | wc -l)

  [[ "$not_ready" -eq 0 ]]
}

check_kind_deployments() {
  vt_require_cmd kubectl

  local context="${KIND_CONTEXT:-kind-dev}"
  local namespace="${KIND_NAMESPACE:-vipertrade}"
  local deployments=(market-data analytics strategy executor monitor api ai-analyst web)
  local failed=0

  for dep in "${deployments[@]}"; do
    if ! kubectl --context "$context" -n "$namespace" rollout status "deployment/$dep" --timeout=30s >/dev/null 2>&1; then
      failed=1
    fi
  done

  [[ "$failed" -eq 0 ]]
}

check_kind_subscribers() {
  vt_require_cmd kubectl

  local context="${KIND_CONTEXT:-kind-dev}"
  local namespace="${KIND_NAMESPACE:-vipertrade}"

  local output
  output=$(kubectl --context "$context" -n "$namespace" exec vipertrade-redis -- redis-cli PUBSUB NUMSUB viper:market-signals viper:decisions 2>/dev/null || true)

  [[ -n "$output" ]]
}

check_kind_events() {
  vt_require_cmd kubectl

  local context="${KIND_CONTEXT:-kind-dev}"
  local namespace="${KIND_NAMESPACE:-vipertrade}"

  local strategy_logs executor_logs
  strategy_logs=$(kubectl --context "$context" -n "$namespace" logs --tail=100 vipertrade-strategy 2>&1 || true)
  executor_logs=$(kubectl --context "$context" -n "$namespace" logs --tail=100 vipertrade-executor 2>&1 || true)

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

  print_header

  case "$MODE" in
    bridge)
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
      ;;

    kind)
      KIND_CONTEXT="${KIND_CONTEXT:-kind-dev}"
      KIND_NAMESPACE="${KIND_NAMESPACE:-vipertrade}"

      case "$ACTION" in
        start)
          print_fail "Use 'make kind-build-images' + 'make kind-deploy' to start Kind stack"
          exit 1
          ;;
        health)
          run_check "Kind cluster accessible" check_kind_cluster
          run_check "Pods ready" check_kind_pods_ready
          run_check "Deployments available" check_kind_deployments
          ;;
        subscribers)
          run_check "Redis subscribers (kubectl exec)" check_kind_subscribers
          ;;
        events)
          run_check "Recent event flow (kubectl logs)" check_kind_events
          ;;
        all)
          run_check "Kind cluster accessible" check_kind_cluster
          run_check "Pods ready" check_kind_pods_ready
          run_check "Deployments available" check_kind_deployments
          run_check "Redis subscribers (kubectl exec)" check_kind_subscribers
          run_check "Recent event flow (kubectl logs)" check_kind_events
          ;;
        *)
          print_fail "Unrecognized action '$ACTION'"
          show_help
          exit 1
          ;;
      esac
      ;;

    *)
      print_fail "Unrecognized mode '$MODE'"
      show_help
      exit 1
      ;;
  esac

  print_summary
  [[ "$FAIL_COUNT" -eq 0 ]]
}

main "$@"
