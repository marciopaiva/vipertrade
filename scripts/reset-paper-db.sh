#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$SCRIPT_DIR/lib/common.sh"
. "$SCRIPT_DIR/container-runtime.sh"

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
COMPOSE_FILE="${COMPOSE_FILE:-$ROOT_DIR/compose/docker-compose.yml}"
POSTGRES_USER="${POSTGRES_USER:-viper}"
POSTGRES_DB="${POSTGRES_DB:-vipertrade}"

cd "$ROOT_DIR"

show_help() {
  vt_print_header "ViperTrade - Data Reset Paper DB"
  echo ""
  echo "Usage:"
  echo "  ./scripts/reset-paper-db.sh [--yes]"
  echo ""
  echo "Make target:"
  echo "  make data-reset-paper-db"
}

run_psql() {
  compose_exec_t postgres \
    psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" "$@"
}

run_api() {
  compose_exec_t api sh -lc "$1"
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" || "${1:-}" == "help" ]]; then
  show_help
  exit 0
fi

vt_print_header "ViperTrade - Data Reset Paper DB"
echo "Compose: $COMPOSE_FILE"
echo ""

BEFORE_COUNTS="$(run_psql -At -F '|' -c "SELECT
  (SELECT COUNT(*) FROM trades),
  (SELECT COUNT(*) FROM trades WHERE status = 'open'),
  (SELECT COUNT(*) FROM position_snapshots),
  (SELECT COUNT(*) FROM strategy_decision_audit);")"

IFS='|' read -r BEFORE_TRADES BEFORE_OPEN BEFORE_SNAPSHOTS BEFORE_AUDIT <<< "$BEFORE_COUNTS"

echo "Before reset:"
echo "- trades: $BEFORE_TRADES"
echo "- open trades: $BEFORE_OPEN"
echo "- position_snapshots: $BEFORE_SNAPSHOTS"
echo "- strategy_decision_audit: $BEFORE_AUDIT"
echo ""

if [[ "${1:-}" != "--yes" ]]; then
  echo -e "${VT_YELLOW}Tip:${VT_NC} for non-interactive usage, run: ./scripts/reset-paper-db.sh --yes"
  read -r -p "Proceed with the reset? [y/N] " CONFIRM
  if [[ ! "$CONFIRM" =~ ^[Yy]$ ]]; then
    echo "Aborted."
    exit 0
  fi
fi

run_psql -c "BEGIN;
DELETE FROM position_snapshots;
DELETE FROM trades;
DELETE FROM strategy_decision_audit;
COMMIT;"

AFTER_COUNTS="$(run_psql -At -F '|' -c "SELECT
  (SELECT COUNT(*) FROM trades),
  (SELECT COUNT(*) FROM trades WHERE status = 'open'),
  (SELECT COUNT(*) FROM position_snapshots),
  (SELECT COUNT(*) FROM strategy_decision_audit);")"

IFS='|' read -r AFTER_TRADES AFTER_OPEN AFTER_SNAPSHOTS AFTER_AUDIT <<< "$AFTER_COUNTS"

STATUS_JSON="$(run_api 'curl -s http://localhost:8080/api/v1/status || true')"

echo ""
vt_ok "Reset completed"
echo "After reset:"
echo "- trades: $AFTER_TRADES"
echo "- open trades: $AFTER_OPEN"
echo "- position_snapshots: $AFTER_SNAPSHOTS"
echo "- strategy_decision_audit: $AFTER_AUDIT"
echo ""

echo "Runtime status:"
echo "$STATUS_JSON"
