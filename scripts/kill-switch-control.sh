#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

ACTION="${1:-}"
API_URL="${API_URL:-http://localhost:8080}"
OPERATOR_TOKEN="${OPERATOR_API_TOKEN:-}"
OPERATOR_ID="${OPERATOR_ID:-local-ops}"
REASON="${REASON:-manual_control}"
DB_CONTAINER="${DB_CONTAINER:-vipertrade-postgres}"
DB_USER="${DB_USER:-viper}"
DB_NAME="${DB_NAME:-vipertrade}"
VERIFY_DB="${VERIFY_DB:-1}"
. "$SCRIPT_DIR/lib/common.sh"
. "$SCRIPT_DIR/container-runtime.sh"

usage() {
  vt_print_header "ViperTrade - Kill Switch Control"
  echo ""
  echo "Usage:"
  echo "  ./scripts/kill-switch-control.sh status"
  echo "  REASON=\"incident\" OPERATOR_API_TOKEN=... ./scripts/kill-switch-control.sh enable"
  echo "  REASON=\"recovered\" OPERATOR_API_TOKEN=... ./scripts/kill-switch-control.sh disable"
  echo ""
  echo "Actions:"
  echo "  status   - show the current kill switch state"
  echo "  enable   - enable the global execution block"
  echo "  disable  - disable the global execution block"
  echo ""
  echo "Environment:"
  echo "  API_URL            API base URL (default: http://localhost:8080)"
  echo "  OPERATOR_API_TOKEN Required for enable/disable"
  echo "  OPERATOR_ID        Operator identifier for the audit event (default: local-ops)"
  echo "  REASON             Reason for the operation"
  echo "  VERIFY_DB          1 to verify in PostgreSQL (default), 0 to skip"
  echo "  DB_CONTAINER       PostgreSQL container name (default: vipertrade-postgres)"
  echo "  DB_USER            PostgreSQL user (default: viper)"
  echo "  DB_NAME            Database name (default: vipertrade)"
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "ERROR: required command not found: $1" >&2
    exit 1
  }
}

print_json_pretty() {
  if command -v jq >/dev/null 2>&1; then
    jq .
  elif command -v python3 >/dev/null 2>&1; then
    python3 -m json.tool
  else
    cat
  fi
}

api_status() {
  need_cmd curl
  curl -fsS "${API_URL}/api/v1/status"
}

api_set_kill_switch() {
  local enabled="$1"
  need_cmd curl

  if [[ -z "${OPERATOR_TOKEN}" ]]; then
    echo "ERROR: OPERATOR_API_TOKEN is required for enable/disable" >&2
    exit 1
  fi

  curl -fsS -X POST "${API_URL}/api/v1/control/kill-switch" \
    -H "content-type: application/json" \
    -H "x-operator-token: ${OPERATOR_TOKEN}" \
    -H "x-operator-id: ${OPERATOR_ID}" \
    -d "{\"enabled\":${enabled},\"reason\":\"${REASON}\"}"
}

db_verify_latest_event() {
  [[ "${VERIFY_DB}" == "1" ]] || return 0
  echo "[db] latest api_kill_switch_set event:"
  container_exec_i "${DB_CONTAINER}" psql -U "${DB_USER}" -d "${DB_NAME}" -At -F '|' -c \
    "SELECT event_type,
            severity,
            data->>'enabled',
            data->>'reason',
            data->>'actor',
            to_char(timestamp,'YYYY-MM-DD HH24:MI:SS')
     FROM system_events
     WHERE event_type='api_kill_switch_set'
     ORDER BY timestamp DESC
     LIMIT 1;"
}

case "${ACTION}" in
  status)
    vt_print_header "ViperTrade - Kill Switch Control"
    echo "[api] querying /api/v1/status"
    api_status | print_json_pretty
    db_verify_latest_event
    ;;
  enable)
    vt_print_header "ViperTrade - Kill Switch Control"
    echo "[api] enabling kill switch"
    api_set_kill_switch true | print_json_pretty
    db_verify_latest_event
    ;;
  disable)
    vt_print_header "ViperTrade - Kill Switch Control"
    echo "[api] disabling kill switch"
    api_set_kill_switch false | print_json_pretty
    db_verify_latest_event
    ;;
  -h|--help|help|"")
    usage
    ;;
  *)
    echo "ERROR: invalid action '${ACTION}'" >&2
    usage
    exit 1
    ;;
esac
