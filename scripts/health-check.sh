#!/bin/bash
set -euo pipefail

# ═══════════════════════════════════════════════════════════════════════════
# ViperTrade Health Check Script
# Uso: ./health-check.sh [all|postgres|redis|strategy|executor|api|web]
# ═══════════════════════════════════════════════════════════════════════════

GREEN="\033[0;32m"
RED="\033[0;31m"
YELLOW="\033[1;33m"
CYAN="\033[0;36m"
NC="\033[0m"

# Default service
SERVICE="${1:-all}"

# Database config
DB_USER="${POSTGRES_USER:-viper}"
DB_NAME="${POSTGRES_DB:-vipertrade}"

# Container runtime
CONTAINER_ENGINE="docker"
if command -v docker >/dev/null 2>&1; then
  CONTAINER_ENGINE="docker"
elif command -v podman >/dev/null 2>&1; then
  CONTAINER_ENGINE="podman"
fi

# Helper functions
print_header() {
  echo -e "${GREEN}ViperTrade - Health Check${NC}"
  echo "============================================"
}

print_service() {
  echo -e "${YELLOW}→${NC} Health: $1..."
}

print_ok() {
  echo -e "${GREEN}✓${NC} $1 OK"
}

print_fail() {
  echo -e "${RED}✗${NC} $1 não disponível"
}

# Health check functions
check_postgres() {
  print_service "PostgreSQL"
  if $CONTAINER_ENGINE exec vipertrade-postgres pg_isready -U "$DB_USER" -d "$DB_NAME" >/dev/null 2>&1; then
    print_ok "PostgreSQL"
    return 0
  else
    print_fail "PostgreSQL"
    return 1
  fi
}

check_redis() {
  print_service "Redis"
  if $CONTAINER_ENGINE exec vipertrade-redis redis-cli ping >/dev/null 2>&1; then
    print_ok "Redis"
    return 0
  else
    print_fail "Redis"
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
  check_strategy || failed=1
  check_executor || failed=1
  check_api || failed=1
  check_web || failed=1
  
  return $failed
}

show_help() {
  echo "Uso: $0 [serviço]"
  echo ""
  echo "Serviços disponíveis:"
  echo "  all       - Todos os serviços (padrão)"
  echo "  postgres  - PostgreSQL"
  echo "  redis     - Redis"
  echo "  strategy  - Strategy Service"
  echo "  executor  - Executor Service"
  echo "  api       - API Service"
  echo "  web       - Web Dashboard"
  echo ""
  echo "Exemplos:"
  echo "  $0          # Todos os serviços"
  echo "  $0 redis    # Apenas Redis"
  echo "  $0 api      # Apenas API"
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
  strategy)
    check_strategy
    ;;
  executor)
    check_executor
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
    echo -e "${RED}Erro: Serviço '$SERVICE' não reconhecido${NC}"
    show_help
    exit 1
    ;;
esac
