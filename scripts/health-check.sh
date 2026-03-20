#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$SCRIPT_DIR/lib/common.sh"

SERVICE="${1:-all}"

# Database config
DB_USER="${POSTGRES_USER:-viper}"
DB_NAME="${POSTGRES_DB:-vipertrade}"

require_docker() {
  command -v docker >/dev/null 2>&1
}

# Helper functions
print_header() {
  vt_print_header "ViperTrade - Health Check"
}

print_service() {
  vt_step "Health: $1..."
}

print_ok() {
  vt_ok "$1 OK"
}

print_fail() {
  vt_fail "$1 unavailable"
}

# Health check functions
check_postgres() {
  print_service "PostgreSQL"
  if ! require_docker; then
    print_fail "Docker"
    return 1
  fi

  if docker exec vipertrade-postgres pg_isready -U "$DB_USER" -d "$DB_NAME" >/dev/null 2>&1; then
    print_ok "PostgreSQL"
    return 0
  else
    print_fail "PostgreSQL"
    return 1
  fi
}

check_redis() {
  print_service "Redis"
  if ! require_docker; then
    print_fail "Docker"
    return 1
  fi

  if docker exec vipertrade-redis redis-cli ping >/dev/null 2>&1; then
    print_ok "Redis"
    return 0
  else
    print_fail "Redis"
    return 1
  fi
}

check_market_data() {
  print_service "Market Data"
  if curl -sf http://localhost:8081/health >/dev/null 2>&1; then
    print_ok "Market Data"
    return 0
  else
    print_fail "Market Data"
    return 1
  fi
}

check_analytics() {
  print_service "Analytics"
  if curl -sf http://localhost:8086/health >/dev/null 2>&1; then
    print_ok "Analytics"
    return 0
  else
    print_fail "Analytics"
    return 1
  fi
}

check_strategy() {
  print_service "Strategy"
  if curl -sf http://localhost:8082/health >/dev/null 2>&1; then
    print_ok "Strategy"
    return 0
  else
    print_fail "Strategy"
    return 1
  fi
}

check_executor() {
  print_service "Executor"
  if curl -sf http://localhost:8083/health >/dev/null 2>&1; then
    print_ok "Executor"
    return 0
  else
    print_fail "Executor"
    return 1
  fi
}

check_monitor() {
  print_service "Monitor"
  if curl -sf http://localhost:8084/health >/dev/null 2>&1; then
    print_ok "Monitor"
    return 0
  else
    print_fail "Monitor"
    return 1
  fi
}

check_backtest() {
  print_service "Backtest"
  if curl -sf http://localhost:8085/health >/dev/null 2>&1; then
    print_ok "Backtest"
    return 0
  else
    print_fail "Backtest"
    return 1
  fi
}

check_api() {
  print_service "API"
  if curl -sf http://localhost:8080/health >/dev/null 2>&1; then
    print_ok "API"
    return 0
  else
    print_fail "API"
    return 1
  fi
}

check_web() {
  print_service "Web"
  if curl -sf http://localhost:3000 >/dev/null 2>&1; then
    print_ok "Web"
    return 0
  else
    print_fail "Web"
    return 1
  fi
}

check_all() {
  local failed=0
  
  check_postgres || failed=1
  check_redis || failed=1
  check_market_data || failed=1
  check_analytics || failed=1
  check_strategy || failed=1
  check_executor || failed=1
  check_monitor || failed=1
  check_backtest || failed=1
  check_api || failed=1
  check_web || failed=1
  
  return $failed
}

show_help() {
  print_header
  echo ""
  echo "Usage: $0 [service]"
  echo ""
  echo "Services available in the script:"
  echo "  all         - all services (default)"
  echo "  postgres   - PostgreSQL"
  echo "  redis      - Redis"
  echo "  market-data - Market Data service (8081)"
  echo "  analytics  - Analytics service (8086)"
  echo "  strategy   - Strategy service (8082)"
  echo "  executor   - Executor service (8083)"
  echo "  monitor    - Monitor service (8084)"
  echo "  backtest   - Backtest service (8085)"
  echo "  api        - API service (8080)"
  echo "  web        - Web dashboard (3000)"
  echo ""
  echo "Examples:"
  echo "  $0"
  echo "  $0 redis"
  echo "  $0 api"
  echo "  $0 strategy"
}

# Main
cd "$(dirname "$0")/.."

case "$SERVICE" in
  all)
    print_header
    check_all
    ;;
  postgres)
    check_postgres
    ;;
  redis)
    check_redis
    ;;
  market-data)
    check_market_data
    ;;
  analytics)
    check_analytics
    ;;
  strategy)
    check_strategy
    ;;
  executor)
    check_executor
    ;;
  monitor)
    check_monitor
    ;;
  backtest)
    check_backtest
    ;;
  api)
    check_api
    ;;
  web)
    check_web
    ;;
  help|-h|--help)
    show_help
    ;;
  *)
    vt_fail "Unrecognized service '$SERVICE'"
    show_help
    exit 1
    ;;
esac
