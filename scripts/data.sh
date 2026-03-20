#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$SCRIPT_DIR/lib/common.sh"

SERVICE="${1:-all}"
ACTION="${2:-status}"

# Database config
DB_USER="${POSTGRES_USER:-viper}"
DB_NAME="${POSTGRES_DB:-vipertrade}"
DB_PORT="${DB_PORT:-5432}"

require_docker() {
  if ! command -v docker >/dev/null 2>&1; then
    print_fail "Docker not found"
    exit 1
  fi
}

# Helper functions
print_header() {
  vt_print_header "ViperTrade - Data Management"
}

print_service() {
  vt_step "$1: $2..."
}

print_ok() {
  vt_ok "$1"
}

print_fail() {
  vt_fail "$1"
}

# PostgreSQL functions
postgres_status() {
  print_service "PostgreSQL" "Status"
  require_docker
  
  if docker ps --filter "name=vipertrade-postgres" --format "{{.Status}}" | grep -q "Up"; then
    print_ok "PostgreSQL is running"
    
    # Check health
    if docker exec vipertrade-postgres pg_isready -U "$DB_USER" -d "$DB_NAME" >/dev/null 2>&1; then
      print_ok "PostgreSQL accepting connections"
    else
      print_fail "PostgreSQL not accepting connections"
    fi
  else
    print_fail "PostgreSQL is not running"
  fi
}

postgres_restart() {
  print_service "PostgreSQL" "Restarting"
  require_docker
  docker restart vipertrade-postgres
  sleep 3
  
  if docker exec vipertrade-postgres pg_isready -U "$DB_USER" >/dev/null 2>&1; then
    print_ok "PostgreSQL restarted successfully"
  else
    print_fail "Failed to restart PostgreSQL"
    return 1
  fi
}

postgres_logs() {
  print_service "PostgreSQL" "Logs (last 50 lines)"
  require_docker
  docker logs --tail 50 vipertrade-postgres
}

postgres_shell() {
  print_service "PostgreSQL" "Opening shell"
  require_docker
  docker exec -it vipertrade-postgres psql -U "$DB_USER" -d "$DB_NAME"
}

postgres_backup() {
  local backup_file="backup_postgres_$(date +%Y%m%d_%H%M%S).sql"
  print_service "PostgreSQL" "Creating backup: $backup_file"
  
  require_docker
  docker exec vipertrade-postgres pg_dump -U "$DB_USER" -d "$DB_NAME" > "$backup_file"
  
  if [[ -f "$backup_file" ]]; then
    local size=$(du -h "$backup_file" | cut -f1)
    print_ok "Backup created: $backup_file ($size)"
  else
    print_fail "Failed to create backup"
    return 1
  fi
}

postgres_health() {
  require_docker
  if docker exec vipertrade-postgres pg_isready -U "$DB_USER" -d "$DB_NAME" >/dev/null 2>&1; then
    print_ok "PostgreSQL OK"
    return 0
  else
    print_fail "PostgreSQL unavailable"
    return 1
  fi
}

# Redis functions
redis_status() {
  print_service "Redis" "Status"
  require_docker
  
  if docker ps --filter "name=vipertrade-redis" --format "{{.Status}}" | grep -q "Up"; then
    print_ok "Redis is running"
    
    # Check health
    if docker exec vipertrade-redis redis-cli ping >/dev/null 2>&1; then
      print_ok "Redis responding to PING"
      
      # Memory info
      local memory=$(docker exec vipertrade-redis redis-cli INFO memory | grep "used_memory_human" | cut -d: -f2 | tr -d '\r')
      echo -e "  ${VT_CYAN}Memory:${VT_NC} $memory"
      
      # Keys count
      local keys=$(docker exec vipertrade-redis redis-cli DBSIZE | cut -d: -f2 | tr -d '\r')
      echo -e "  ${VT_CYAN}Keys:${VT_NC} $keys"
    else
      print_fail "Redis not responding"
    fi
  else
    print_fail "Redis is not running"
  fi
}

redis_restart() {
  print_service "Redis" "Restarting"
  require_docker
  docker restart vipertrade-redis
  sleep 2
  
  if docker exec vipertrade-redis redis-cli ping >/dev/null 2>&1; then
    print_ok "Redis restarted successfully"
  else
    print_fail "Failed to restart Redis"
    return 1
  fi
}

redis_logs() {
  print_service "Redis" "Logs (last 50 lines)"
  require_docker
  docker logs --tail 50 vipertrade-redis
}

redis_shell() {
  print_service "Redis" "Opening CLI"
  require_docker
  docker exec -it vipertrade-redis redis-cli
}

redis_backup() {
  print_service "Redis" "Creating RDB snapshot"
  require_docker
  docker exec vipertrade-redis redis-cli BGSAVE
  
  sleep 2
  if docker exec vipertrade-redis redis-cli LASTSAVE >/dev/null 2>&1; then
    print_ok "Redis RDB snapshot started"
  else
    print_fail "Failed to create snapshot"
    return 1
  fi
}

redis_health() {
  require_docker
  if docker exec vipertrade-redis redis-cli ping >/dev/null 2>&1; then
    print_ok "Redis OK"
    return 0
  else
    print_fail "Redis unavailable"
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
  echo "Usage: $0 [service] [action]"
  echo ""
  echo "Services:"
  echo "  postgres  - PostgreSQL database"
  echo "  redis     - Redis cache"
  echo "  all       - all services (default)"
  echo ""
  echo "Actions:"
  echo "  status   - show service status"
  echo "  health   - run health checks"
  echo "  restart  - restart the service"
  echo "  logs     - show logs"
  echo "  shell    - open the shell/CLI"
  echo "  backup   - create a backup"
  echo ""
  echo "Examples:"
  echo "  $0 postgres status"
  echo "  $0 redis shell"
  echo "  $0 all backup"
  echo "  $0 all health"
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
      *) echo -e "${VT_RED}ERROR: unrecognized action '$ACTION'${VT_NC}"; show_help; exit 1 ;;
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
      *) echo -e "${VT_RED}ERROR: unrecognized action '$ACTION'${VT_NC}"; show_help; exit 1 ;;
    esac
    ;;
  
  all)
    case "$ACTION" in
      status) all_status ;;
      health) all_health ;;
      restart) all_restart ;;
      logs) all_logs ;;
      backup) all_backup ;;
      *) echo -e "${VT_RED}ERROR: unrecognized action '$ACTION'${VT_NC}"; show_help; exit 1 ;;
    esac
    ;;
  
  help|-h|--help)
    show_help
    ;;
  
  *)
    echo -e "${VT_RED}ERROR: unrecognized service '$SERVICE'${VT_NC}"
    show_help
    exit 1
    ;;
esac
