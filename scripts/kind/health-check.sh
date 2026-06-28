#!/usr/bin/env bash
# ViperTrade - Kind Health Check

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/../lib/common.sh"

KIND_CONTEXT="${KIND_CONTEXT:-kind-dev}"
KIND_NAMESPACE="${KIND_NAMESPACE:-vipertrade}"

vt_require_cmd kubectl

PASS_COUNT=0
FAIL_COUNT=0

print_header() { vt_print_header "ViperTrade Kind - Health Check"; }
print_step()  { vt_step "$1"; }
print_ok()    { PASS_COUNT=$((PASS_COUNT+1)); vt_ok "$1"; }
print_fail()  { FAIL_COUNT=$((FAIL_COUNT+1)); vt_fail "$1"; }

check_cluster_accessible() {
  print_step "Cluster accessibility"
  kubectl cluster-info --context "$KIND_CONTEXT" >/dev/null 2>&1 && print_ok "Cluster accessible" || print_fail "Cluster not accessible ($KIND_CONTEXT)"
}

check_pods_ready() {
  print_step "All pods ready?"
  local not_ready
  not_ready=$(kubectl --context "$KIND_CONTEXT" -n "$KIND_NAMESPACE" get pods -o jsonpath='{range .items[?(@.status.containerStatuses[0].ready!=true)]}{.metadata.name}{"\n"}{end}' 2>/dev/null | wc -l)

  if [[ "$not_ready" -eq 0 ]]; then
    print_ok "All pods ready"
    return 0
  else
    print_fail "Pods not ready (count=$not_ready)"
    kubectl --context "$KIND_CONTEXT" -n "$KIND_NAMESPACE" get pods
    return 1
  fi
}

check_deployments_available() {
  print_step "Deployments available"
  local deployments=(market-data analytics strategy executor monitor api ai-analyst web)
  local failed=0

  for dep in "${deployments[@]}"; do
    if ! kubectl --context "$KIND_CONTEXT" -n "$KIND_NAMESPACE" rollout status "deployment/$dep" --timeout=10s >/dev/null 2>&1; then
      print_fail "Deployment $dep not available"
      failed=1
    fi
  done

  if [[ "$failed" -eq 0 ]]; then
    print_ok "All deployments available"
    return 0
  else
    return 1
  fi
}

check_services_exist() {
  print_step "Services exist"
  local services=(postgres redis market-data analytics strategy executor monitor api ai-analyst web)
  local missing=0

  for svc in "${services[@]}"; do
    if ! kubectl --context "$KIND_CONTEXT" -n "$KIND_NAMESPACE" get svc "$svc" >/dev/null 2>&1; then
      print_fail "Service $svc missing"
      missing=1
    fi
  done

  if [[ "$missing" -eq 0 ]]; then
    print_ok "All services present"
    return 0
  else
    return 1
  fi
}

check_postgres_ready() {
  print_step "PostgreSQL ready"
  kubectl --context "$KIND_CONTEXT" -n "$KIND_NAMESPACE" exec deployment/postgres -- pg_isready -U viper -d vipertrade -h 127.0.0.1 >/dev/null 2>&1 && print_ok "PostgreSQL accepting connections" || print_fail "PostgreSQL not ready"
}

check_redis_ready() {
  print_step "Redis ready"
  kubectl --context "$KIND_CONTEXT" -n "$KIND_NAMESPACE" exec deployment/redis -- redis-cli ping >/dev/null 2>&1 && print_ok "Redis responding to PING" || print_fail "Redis not responding"
}

print_summary() {
  echo ""
  echo -e "${VT_CYAN}Summary:${VT_NC} PASS=$PASS_COUNT FAIL=$FAIL_COUNT"
}

show_help() {
  print_header
  echo ""
  echo "Usage: $0 [all|cluster|pods|deployments|services|db]"
  echo "  KIND_CONTEXT, KIND_NAMESPACE env vars can override defaults"
}

main() {
  local check_target="${1:-all}"

  if [[ "$check_target" == "help" || "$check_target" == "-h" || "$check_target" == "--help" ]]; then
    show_help
    exit 0
  fi

  print_header

  case "$check_target" in
    all)
      check_cluster_accessible && \
      check_pods_ready && \
      check_deployments_available && \
      check_services_exist && \
      check_postgres_ready && \
      check_redis_ready
      ;;
    cluster)
      check_cluster_accessible
      ;;
    pods)
      check_pods_ready
      ;;
    deployments)
      check_deployments_available
      ;;
    services)
      check_services_exist
      ;;
    db)
      check_postgres_ready && check_redis_ready
      ;;
    *)
      print_fail "Unrecognized check '$check_target'"
      show_help
      exit 1
      ;;
  esac

  print_summary
  [[ "$FAIL_COUNT" -eq 0 ]]
}

main "$@"
