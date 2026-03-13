#!/usr/bin/env bash
set -euo pipefail

ACTION="${1:-}"
API_URL="${API_URL:-http://localhost:8080}"
OPERATOR_TOKEN="${OPERATOR_API_TOKEN:-}"
OPERATOR_ID="${OPERATOR_ID:-local-ops}"
REASON="${REASON:-manual_control}"
DB_CONTAINER="${DB_CONTAINER:-vipertrade-postgres}"
DB_USER="${DB_USER:-viper}"
DB_NAME="${DB_NAME:-vipertrade}"
VERIFY_DB="${VERIFY_DB:-1}"
. "$(dirname "$0")/container-runtime.sh"

usage() {
  cat <<'USAGE'
Usage:
  ./scripts/kill-switch-control.sh status
  REASON="incident" OPERATOR_API_TOKEN=... ./scripts/kill-switch-control.sh enable
  REASON="recovered" OPERATOR_API_TOKEN=... ./scripts/kill-switch-control.sh disable

Environment:
  API_URL            API base URL (default: http://localhost:8080)
  OPERATOR_API_TOKEN Required for enable/disable
  OPERATOR_ID        Operator identifier in audit event (default: local-ops)
  REASON             Reason for enable/disable operation
  VERIFY_DB          1 to run DB verification query (default), 0 to skip
  DB_CONTAINER       Postgres container name (default: vipertrade-postgres)
  DB_USER            Postgres user (default: viper)
  DB_NAME            Postgres DB name (default: vipertrade)
USAGE
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
    echo "[api] /api/v1/status"
    api_status | print_json_pretty
    db_verify_latest_event
    ;;
  enable)
    echo "[api] enabling kill-switch"
    api_set_kill_switch true | print_json_pretty
    db_verify_latest_event
    ;;
  disable)
    echo "[api] disabling kill-switch"
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
