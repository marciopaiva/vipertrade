#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$SCRIPT_DIR/lib/common.sh"

SERVICE="${1:-all}"
DB_USER="${POSTGRES_USER:-viper}"
DB_NAME="${POSTGRES_DB:-vipertrade}"

require_container() { vt_container_available; }

print_header() { vt_print_header "ViperTrade - Health Check"; }
print_service() { vt_step "Health: $1..."; }
print_ok() { vt_ok "$1 OK"; }
print_fail() { vt_fail "$1 unavailable"; }

check_container() {
  local name="$1"
  shift
  print_service "$name"
  require_container || { print_fail "Container engine"; return 1; }
  vt_container exec "$name" "$@" >/dev/null 2>&1 && print_ok "$name" || { print_fail "$name"; return 1; }
}

check_http() {
  local name="$1" url="$2"
  print_service "$name"
  curl -sf "$url" >/dev/null 2>&1 && print_ok "$name" || { print_fail "$name"; return 1; }
}

check_postgres() { check_container vipertrade-postgres pg_isready -U "$DB_USER" -d "$DB_NAME"; }
check_redis() { check_container vipertrade-redis redis-cli ping; }

check_market_data() { check_http "Market Data" "http://localhost:8081/health"; }
check_analytics()  { check_http "Analytics" "http://localhost:8086/health"; }
check_strategy()   { check_http "Strategy" "http://localhost:8082/health"; }
check_executor()   { check_http "Executor" "http://localhost:8083/health"; }
check_monitor()    { check_http "Monitor" "http://localhost:8084/health"; }
check_backtest()   { check_http "Backtest" "http://localhost:8085/health"; }
check_api()        { check_http "API" "http://localhost:8080/health"; }
check_web()        { check_http "Web" "http://localhost:3000"; }

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
  echo "Services:"
  echo "  all         - all services (default)"
  echo "  postgres    - PostgreSQL"
  echo "  redis       - Redis"
  echo "  market-data - Market Data (8081)"
  echo "  analytics   - Analytics (8086)"
  echo "  strategy    - Strategy (8082)"
  echo "  executor    - Executor (8083)"
  echo "  monitor     - Monitor (8084)"
  echo "  backtest    - Backtest (8085)"
  echo "  api         - API (8080)"
  echo "  web         - Web dashboard (3000)"
  echo ""
  echo "Examples: $0, $0 redis, $0 api"
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
