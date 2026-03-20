#!/bin/bash
set -euo pipefail

# ═══════════════════════════════════════════════════════════════════════════
# ViperTrade Runtime Validation Script
# Uso: ./scripts/validate-runtime.sh [bridge|host] [start|check|subscribers|events|all]
# ═══════════════════════════════════════════════════════════════════════════

GREEN="\033[0;32m"
RED="\033[0;31m"
YELLOW="\033[1;33m"
CYAN="\033[0;36m"
NC="\033[0m"

# Defaults
MODE="${1:-bridge}"
ACTION="${2:-all}"

BUILD="${BUILD:-0}"
LOG_WINDOW="${LOG_WINDOW:-120s}"

# Handle help first (before MODE validation)
if [[ "$MODE" == "help" || "$MODE" == "-h" || "$MODE" == "--help" ]]; then
  echo -e "${GREEN}ViperTrade - Runtime Validation${NC}"
  echo "============================================"
  echo ""
  echo "Uso: $0 [modo] [ação]"
  echo ""
  echo "Modos:"
  echo "  bridge  - Docker bridge network (padrão)"
  echo "  host    - Docker host network"
  echo ""
  echo "Ações:"
  echo "  start       - Iniciar stack"
  echo "  check       - Health check"
  echo "  subscribers - Validar Redis subscribers"
  echo "  events      - Validar event flow"
  echo "  all         - Validação completa (padrão)"
  echo ""
  echo "Exemplos:"
  echo "  $0 bridge all        # Validação completa em bridge mode"
  echo "  $0 bridge check      # Apenas health check"
  echo "  $0 host subscribers  # Validar subscribers em host mode"
  echo ""
  echo "Variáveis de ambiente:"
  echo "  BUILD=1        - Build das imagens antes de iniciar"
  echo "  LOG_WINDOW=60s - Janela de tempo para logs (default: 120s)"
  exit 0
fi

# Container runtime
CONTAINER_ENGINE="docker"
if command -v docker >/dev/null 2>&1; then
  CONTAINER_ENGINE="docker"
elif command -v podman >/dev/null 2>&1; then
  CONTAINER_ENGINE="podman"
fi

# Compose script
case "$MODE" in
  bridge)
    COMPOSE_SCRIPT="./scripts/compose.sh"
    ;;
  host)
    COMPOSE_SCRIPT="./scripts/compose-host.sh"
    ;;
  *)
    echo -e "${RED}Erro: mode deve ser 'bridge' ou 'host'${NC}"
    exit 1
    ;;
esac

# Helper functions
print_header() {
  echo -e "${GREEN}ViperTrade - Runtime Validation (${MODE})${NC}"
  echo "============================================"
}

print_step() {
  echo -e "${YELLOW}→${NC} $1..."
}

print_ok() {
  echo -e "${GREEN}✓${NC} $1"
}

print_fail() {
  echo -e "${RED}✗${NC} $1"
}

# Validation functions
validate_start() {
  print_step "Bringing stack down"
  $COMPOSE_SCRIPT down || true
  
  print_step "Starting stack"
  if [[ "$BUILD" == "1" ]]; then
    $COMPOSE_SCRIPT up -d --build
  else
    $COMPOSE_SCRIPT up -d
  fi
  
  print_ok "Stack iniciada"
}

validate_check() {
  print_step "Health check"
  
  local healthy=0
  for _ in $(seq 1 20); do
    if ./scripts/health-check.sh >/tmp/viper_health.log 2>&1; then
      healthy=1
      break
    fi
    sleep 3
  done
  
  if [[ "$healthy" != "1" ]]; then
    print_fail "Health-check não passou no tempo esperado"
    tail -n 80 /tmp/viper_health.log || true
    return 1
  fi
  
  print_ok "Health-check passou"
  return 0
}

validate_subscribers() {
  print_step "Redis subscribers"
  
  local nums=$($CONTAINER_ENGINE exec vipertrade-redis redis-cli PUBSUB NUMSUB viper:market_data viper:decisions)
  local md_sub=$(echo "$nums" | awk 'NR==2 {print $1}')
  local dec_sub=$(echo "$nums" | awk 'NR==4 {print $1}')
  
  if [[ -z "${md_sub:-}" || -z "${dec_sub:-}" ]]; then
    print_fail "Falha ao parsear Redis NUMSUB"
    echo "$nums"
    return 1
  fi
  
  if (( md_sub < 1 )); then
    print_fail "viper:market_data sem subscribers"
    return 1
  fi
  
  if (( dec_sub < 1 )); then
    print_fail "viper:decisions sem subscribers"
    return 1
  fi
  
  print_ok "Redis subscribers market_data=${md_sub} decisions=${dec_sub}"
  return 0
}

validate_events() {
  print_step "Event flow validation"
  
  local strategy_events=$($CONTAINER_ENGINE logs --since "$LOG_WINDOW" vipertrade-strategy 2>&1 | grep -c "Published decision event" || true)
  local executor_events=$($CONTAINER_ENGINE logs --since "$LOG_WINDOW" vipertrade-executor 2>&1 | grep -c "Executor received decision event" || true)
  
  if (( strategy_events < 1 )); then
    print_fail "Strategy não produziu decision events"
    return 1
  fi
  
  if (( executor_events < 1 )); then
    print_fail "Executor não consumiu decision events"
    return 1
  fi
  
  print_ok "Event flow strategy=${strategy_events} executor=${executor_events}"
  return 0
}

validate_all() {
  local failed=0
  
  validate_start || failed=1
  validate_check || failed=1
  validate_subscribers || failed=1
  validate_events || failed=1
  
  if [[ "$failed" == "0" ]]; then
    echo ""
    print_ok "SUCCESS: runtime validation passed (${MODE})"
  fi
  
  return $failed
}

# Main
cd "$(dirname "$0")/.."

if [[ ! -x "$COMPOSE_SCRIPT" ]]; then
  echo -e "${RED}Erro: $COMPOSE_SCRIPT não encontrado/não executável${NC}"
  exit 1
fi

case "$ACTION" in
  start)
    validate_start
    ;;
  check)
    validate_check
    ;;
  subscribers)
    validate_subscribers
    ;;
  events)
    validate_events
    ;;
  all)
    validate_all
    ;;
  help|-h|--help)
    show_help
    ;;
  *)
    echo -e "${RED}Erro: Ação '$ACTION' não reconhecida${NC}"
    show_help
    exit 1
    ;;
esac
