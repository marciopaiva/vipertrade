#!/usr/bin/env bash
set -euo pipefail

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
COMPOSE_FILE="${COMPOSE_FILE:-$ROOT_DIR/compose/docker-compose.yml}"
POSTGRES_USER="${POSTGRES_USER:-viper}"
POSTGRES_DB="${POSTGRES_DB:-vipertrade}"

cd "$ROOT_DIR"

if ! command -v docker >/dev/null 2>&1; then
  echo -e "${RED}ERROR:${NC} docker não encontrado"
  exit 1
fi

run_psql() {
  docker compose -f "$COMPOSE_FILE" exec -T postgres \
    psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" "$@"
}

run_api() {
  docker compose -f "$COMPOSE_FILE" exec -T api sh -lc "$1"
}

echo -e "${GREEN}ViperTrade - Reset da base PAPER${NC}"
echo "Compose: $COMPOSE_FILE"
echo

BEFORE_COUNTS="$(run_psql -At -F '|' -c "SELECT
  (SELECT COUNT(*) FROM trades),
  (SELECT COUNT(*) FROM trades WHERE status = 'open'),
  (SELECT COUNT(*) FROM position_snapshots);")"

IFS='|' read -r BEFORE_TRADES BEFORE_OPEN BEFORE_SNAPSHOTS <<< "$BEFORE_COUNTS"

echo "Antes da limpeza:"
echo "- trades: $BEFORE_TRADES"
echo "- open trades: $BEFORE_OPEN"
echo "- position_snapshots: $BEFORE_SNAPSHOTS"
echo

if [[ "${1:-}" != "--yes" ]]; then
  echo -e "${YELLOW}Dica:${NC} para uso não interativo, rode: ./scripts/reset-paper-db.sh --yes"
  read -r -p "Prosseguir com a limpeza? [y/N] " CONFIRM
  if [[ ! "$CONFIRM" =~ ^[Yy]$ ]]; then
    echo "Abortado."
    exit 0
  fi
fi

run_psql -c "BEGIN;
DELETE FROM position_snapshots;
DELETE FROM trades;
COMMIT;"

AFTER_COUNTS="$(run_psql -At -F '|' -c "SELECT
  (SELECT COUNT(*) FROM trades),
  (SELECT COUNT(*) FROM trades WHERE status = 'open'),
  (SELECT COUNT(*) FROM position_snapshots);")"

IFS='|' read -r AFTER_TRADES AFTER_OPEN AFTER_SNAPSHOTS <<< "$AFTER_COUNTS"

STATUS_JSON="$(run_api 'curl -s http://localhost:8080/api/v1/status || true')"

echo
echo -e "${GREEN}Limpeza concluída.${NC}"
echo "Depois da limpeza:"
echo "- trades: $AFTER_TRADES"
echo "- open trades: $AFTER_OPEN"
echo "- position_snapshots: $AFTER_SNAPSHOTS"
echo

echo "Status do runtime:"
echo "$STATUS_JSON"
