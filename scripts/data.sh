#!/bin/bash
set -euo pipefail

# ═══════════════════════════════════════════════════════════════════════════
# ViperTrade Data Management Script
# Uso: ./scripts/data.sh [postgres|redis|all] [status|restart|logs|shell|backup|health]
# ═══════════════════════════════════════════════════════════════════════════

GREEN="\033[0;32m"
RED="\033[0;31m"
YELLOW="\033[1;33m"
CYAN="\033[0;36m"
NC="\033[0m"

# Defaults
SERVICE="${1:-all}"
ACTION="${2:-status}"

# Database config
DB_USER="${POSTGRES_USER:-viper}"
DB_NAME="${POSTGRES_DB:-vipertrade}"
DB_PORT="${DB_PORT:-5432}"

# Container runtime
CONTAINER_ENGINE="docker"
if command -v docker >/dev/null 2>&1; then
  CONTAINER_ENGINE="docker"
elif command -v podman >/dev/null 2>&1; then
  CONTAINER_ENGINE="podman"
fi

# Helper functions
print_header() {
  echo -e "${GREEN}ViperTrade - Data Management${NC}"
  echo "============================================"
}

print_service() {
  echo -e "${YELLOW}→${NC} $1: $2..."
}

print_ok() {
  echo -e "${GREEN}✓${NC} $1"
}

print_fail() {
  echo -e "${RED}✗${NC} $1"
}

# PostgreSQL functions
postgres_status() {
  print_service "PostgreSQL" "Status"
  
  if $CONTAINER_ENGINE ps --filter "name=vipertrade-postgres" --format "{{.Status}}" | grep -q "Up"; then
    print_ok "PostgreSQL está rodando"
    
    # Check health
    if $CONTAINER_ENGINE exec vipertrade-postgres pg_isready -U "$DB_USER" -d "$DB_NAME" >/dev/null 2>&1; then
      print_ok "PostgreSQL accepting connections"
    else
      print_fail "PostgreSQL not accepting connections"
    fi
  else
    print_fail "PostgreSQL não está rodando"
  fi
}

postgres_restart() {
  print_service "PostgreSQL" "Restarting"
  $CONTAINER_ENGINE restart vipertrade-postgres
  sleep 3
  
  if $CONTAINER_ENGINE exec vipertrade-postgres pg_isready -U "$DB_USER" >/dev/null 2>&1; then
    print_ok "PostgreSQL reiniciado com sucesso"
  else
    print_fail "Falha ao reiniciar PostgreSQL"
    return 1
  fi
}

postgres_logs() {
  print_service "PostgreSQL" "Logs (últimas 50 linhas)"
  $CONTAINER_ENGINE logs --tail 50 vipertrade-postgres
}

postgres_shell() {
  print_service "PostgreSQL" "Opening shell"
  $CONTAINER_ENGINE exec -it vipertrade-postgres psql -U "$DB_USER" -d "$DB_NAME"
}

postgres_backup() {
  local backup_file="backup_postgres_$(date +%Y%m%d_%H%M%S).sql"
  print_service "PostgreSQL" "Creating backup: $backup_file"
  
  $CONTAINER_ENGINE exec vipertrade-postgres pg_dump -U "$DB_USER" -d "$DB_NAME" > "$backup_file"
  
  if [[ -f "$backup_file" ]]; then
    local size=$(du -h "$backup_file" | cut -f1)
    print_ok "Backup criado: $backup_file ($size)"
  else
    print_fail "Falha ao criar backup"
    return 1
  fi
}

postgres_health() {
  if $CONTAINER_ENGINE exec vipertrade-postgres pg_isready -U "$DB_USER" -d "$DB_NAME" >/dev/null 2>&1; then
    print_ok "PostgreSQL OK"
    return 0
  else
    print_fail "PostgreSQL não disponível"
    return 1
  fi
}

# Redis functions
redis_status() {
  print_service "Redis" "Status"
  
  if $CONTAINER_ENGINE ps --filter "name=vipertrade-redis" --format "{{.Status}}" | grep -q "Up"; then
    print_ok "Redis está rodando"
    
    # Check health
    if $CONTAINER_ENGINE exec vipertrade-redis redis-cli ping >/dev/null 2>&1; then
      print_ok "Redis responding to PING"
      
      # Memory info
      local memory=$($CONTAINER_ENGINE exec vipertrade-redis redis-cli INFO memory | grep "used_memory_human" | cut -d: -f2 | tr -d '\r')
      echo -e "  ${CYAN}Memória:${NC} $memory"
      
      # Keys count
      local keys=$($CONTAINER_ENGINE exec vipertrade-redis redis-cli DBSIZE | cut -d: -f2 | tr -d '\r')
      echo -e "  ${CYAN}Chaves:${NC} $keys"
    else
      print_fail "Redis not responding"
    fi
  else
    print_fail "Redis não está rodando"
  fi
}

redis_restart() {
  print_service "Redis" "Restarting"
  $CONTAINER_ENGINE restart vipertrade-redis
  sleep 2
  
  if $CONTAINER_ENGINE exec vipertrade-redis redis-cli ping >/dev/null 2>&1; then
    print_ok "Redis reiniciado com sucesso"
  else
    print_fail "Falha ao reiniciar Redis"
    return 1
  fi
}

redis_logs() {
  print_service "Redis" "Logs (últimas 50 linhas)"
  $CONTAINER_ENGINE logs --tail 50 vipertrade-redis
}

redis_shell() {
  print_service "Redis" "Opening CLI"
  $CONTAINER_ENGINE exec -it vipertrade-redis redis-cli
}

redis_backup() {
  print_service "Redis" "Creating RDB snapshot"
  $CONTAINER_ENGINE exec vipertrade-redis redis-cli BGSAVE
  
  sleep 2
  if $CONTAINER_ENGINE exec vipertrade-redis redis-cli LASTSAVE >/dev/null 2>&1; then
    print_ok "Redis RDB snapshot iniciado"
  else
    print_fail "Falha ao criar snapshot"
    return 1
  fi
}

redis_health() {
  if $CONTAINER_ENGINE exec vipertrade-redis redis-cli ping >/dev/null 2>&1; then
    print_ok "Redis OK"
    return 0
  else
    print_fail "Redis não disponível"
    return 1
  fi
}

# All services
all_status() {
  postgres_status
  echo ""
  redis_status
}

all_health() {
  local failed=0
  postgres_health || failed=1
  redis_health || failed=1
  return $failed
}

all_restart() {
  postgres_restart
  redis_restart
}

all_logs() {
  postgres_logs
  echo ""
  redis_logs
}

all_backup() {
  postgres_backup
  redis_backup
}

# Help
show_help() {
  print_header
  echo ""
  echo "Uso: $0 [serviço] [ação]"
  echo ""
  echo "Serviços:"
  echo "  postgres  - PostgreSQL database"
  echo "  redis     - Redis cache"
  echo "  all       - Todos os serviços (padrão)"
  echo ""
  echo "Ações:"
  echo "  status   - Mostrar status do serviço"
  echo "  health   - Verificar saúde"
  echo "  restart  - Reiniciar serviço"
  echo "  logs     - Mostrar logs"
  echo "  shell    - Abrir shell/CLI"
  echo "  backup   - Criar backup"
  echo ""
  echo "Exemplos:"
  echo "  $0 postgres status    # Status do PostgreSQL"
  echo "  $0 redis shell        # Redis CLI"
  echo "  $0 all backup         # Backup de todos"
  echo "  $0 all health         # Health check"
}

# Main
cd "$(dirname "$0")/.."

case "$SERVICE" in
  postgres)
    case "$ACTION" in
      status) postgres_status ;;
      health) postgres_health ;;
      restart) postgres_restart ;;
      logs) postgres_logs ;;
      shell) postgres_shell ;;
      backup) postgres_backup ;;
      *) echo -e "${RED}Erro: Ação '$ACTION' não reconhecida${NC}"; show_help; exit 1 ;;
    esac
    ;;
  
  redis)
    case "$ACTION" in
      status) redis_status ;;
      health) redis_health ;;
      restart) redis_restart ;;
      logs) redis_logs ;;
      shell) redis_shell ;;
      backup) redis_backup ;;
      *) echo -e "${RED}Erro: Ação '$ACTION' não reconhecida${NC}"; show_help; exit 1 ;;
    esac
    ;;
  
  all)
    case "$ACTION" in
      status) all_status ;;
      health) all_health ;;
      restart) all_restart ;;
      logs) all_logs ;;
      backup) all_backup ;;
      *) echo -e "${RED}Erro: Ação '$ACTION' não reconhecida${NC}"; show_help; exit 1 ;;
    esac
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
