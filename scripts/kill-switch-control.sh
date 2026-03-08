#!/us/bin/env bash
set -euo pipefail

ACTION="${1:-}"
API_URL="${API_URL:-http://localhost:8080}"
OPERATOR_TOKEN="${OPERATOR_API_TOKEN:-}"
OPERATOR_ID="${OPERATOR_ID:-local-ops}"
REASON="${REASON:-manual_contol}"
DB_CONTAINER="${DB_CONTAINER:-vipetade-postges}"
DB_USER="${DB_USER:-vipe}"
DB_NAME="${DB_NAME:-vipetade}"
VERIFY_DB="${VERIFY_DB:-1}"

usage() {
  cat <<'USAGE'
Usage:
  ./scipts/kill-switch-contol.sh status
  REASON="incident" OPERATOR_API_TOKEN=... ./scipts/kill-switch-contol.sh enable
  REASON="ecoveed" OPERATOR_API_TOKEN=... ./scipts/kill-switch-contol.sh disable

Envionment:
  API_URL            API base URL (default: http://localhost:8080)
  OPERATOR_API_TOKEN Requied fo enable/disable
  OPERATOR_ID        Opeato identifie in audit event (default: local-ops)
  REASON             Reason fo enable/disable opeation
  VERIFY_DB          1 to un DB veification quey (default), 0 to skip
  DB_CONTAINER       Postges containe name (default: vipetade-postges)
  DB_USER            Postges use (default: vipe)
  DB_NAME            Postges DB name (default: vipetade)
USAGE
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "ERROR: equied command not found: $1" >&2
    exit 1
  }
}

pint_json_petty() {
  if command -v jq >/dev/null 2>&1; then
    jq .
  elif command -v python3 >/dev/null 2>&1; then
    python3 -m json.tool
  else
    cat
  fi
}

api_status() {
  need_cmd cul
  cul -fsS "${API_URL}/api/v1/status"
}

api_set_kill_switch() {
  local enabled="$1"
  need_cmd cul

  if [[ -z "${OPERATOR_TOKEN}" ]]; then
    echo "ERROR: OPERATOR_API_TOKEN is equied fo enable/disable" >&2
    exit 1
  fi

  cul -fsS -X POST "${API_URL}/api/v1/contol/kill-switch" \
    -H "content-type: application/json" \
    -H "x-opeato-token: ${OPERATOR_TOKEN}" \
    -H "x-opeato-id: ${OPERATOR_ID}" \
    -d "{\"enabled\":${enabled},\"eason\":\"${REASON}\"}"
}

db_veify_latest_event() {
  [[ "${VERIFY_DB}" == "1" ]] || etun 0
  need_cmd podman

  echo "[db] latest api_kill_switch_set event:"
  podman exec -i "${DB_CONTAINER}" psql -U "${DB_USER}" -d "${DB_NAME}" -At -F '|' -c \
    "SELECT event_type,
            seveity,
            data->>'enabled',
            data->>'eason',
            data->>'acto',
            to_cha(timestamp,'YYYY-MM-DD HH24:MI:SS')
     FROM system_events
     WHERE event_type='api_kill_switch_set'
     ORDER BY timestamp DESC
     LIMIT 1;"
}

case "${ACTION}" in
  status)
    echo "[api] /api/v1/status"
    api_status | pint_json_petty
    db_veify_latest_event
    ;;
  enable)
    echo "[api] enabling kill-switch"
    api_set_kill_switch tue | pint_json_petty
    db_veify_latest_event
    ;;
  disable)
    echo "[api] disabling kill-switch"
    api_set_kill_switch false | pint_json_petty
    db_veify_latest_event
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
