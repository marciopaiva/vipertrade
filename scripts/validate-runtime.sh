#!/bin/bash
set -euo pipefail

GREEN="\033[0;32m"
RED="\033[0;31m"
YELLOW="\033[1;33m"
NC="\033[0m"

MODE="${1:-bridge}"
BUILD="${BUILD:-0}"
LOG_WINDOW="${LOG_WINDOW:-120s}"

case "$MODE" in
  bridge)
    COMPOSE_SCRIPT="./scripts/compose.sh"
    ;;
  host)
    COMPOSE_SCRIPT="./scripts/compose-host.sh"
    ;;
  *)
    echo -e "${RED}ERROR: mode must be 'bridge' or 'host'${NC}"
    exit 1
    ;;
esac

if [[ ! -x "$COMPOSE_SCRIPT" ]]; then
  echo -e "${RED}ERROR: $COMPOSE_SCRIPT not found/executable${NC}"
  exit 1
fi

echo -e "${GREEN}ViperTrade - Runtime Validation (${MODE})${NC}"
echo "================================================"

echo "Bringing stack down..."
$COMPOSE_SCRIPT down || true

echo "Starting stack..."
if [[ "$BUILD" == "1" ]]; then
  $COMPOSE_SCRIPT up -d --build
else
  $COMPOSE_SCRIPT up -d
fi

healthy=0
for _ in $(seq 1 20); do
  if ./scripts/health-check.sh >/tmp/viper_health.log 2>&1; then
    healthy=1
    break
  fi
  sleep 3
done

if [[ "$healthy" != "1" ]]; then
  echo -e "${RED}ERROR: health-check did not pass in time${NC}"
  tail -n 80 /tmp/viper_health.log || true
  exit 1
fi

echo -e "${GREEN}OK: health-check passed${NC}"

nums=$(podman exec vipertrade-redis redis-cli PUBSUB NUMSUB viper:market_data viper:decisions)
md_sub=$(echo "$nums" | awk 'NR==2 {print $1}')
dec_sub=$(echo "$nums" | awk 'NR==4 {print $1}')

if [[ -z "${md_sub:-}" || -z "${dec_sub:-}" ]]; then
  echo -e "${RED}ERROR: failed to parse Redis NUMSUB output${NC}"
  echo "$nums"
  exit 1
fi

if (( md_sub < 1 )); then
  echo -e "${RED}ERROR: viper:market_data has no subscribers${NC}"
  exit 1
fi
if (( dec_sub < 1 )); then
  echo -e "${RED}ERROR: viper:decisions has no subscribers${NC}"
  exit 1
fi

echo -e "${GREEN}OK: Redis subscribers market_data=${md_sub} decisions=${dec_sub}${NC}"

strategy_events=$(podman logs --since "$LOG_WINDOW" vipertrade-strategy 2>&1 | grep -c "Published decision event" || true)
executor_events=$(podman logs --since "$LOG_WINDOW" vipertrade-executor 2>&1 | grep -c "Executor received decision event" || true)

if (( strategy_events < 1 )); then
  echo -e "${RED}ERROR: strategy produced no decision events${NC}"
  exit 1
fi
if (( executor_events < 1 )); then
  echo -e "${RED}ERROR: executor consumed no decision events${NC}"
  exit 1
fi

echo -e "${GREEN}OK: strategy events=${strategy_events} executor events=${executor_events}${NC}"

echo ""
echo -e "${GREEN}SUCCESS: runtime validation passed (${MODE})${NC}"
