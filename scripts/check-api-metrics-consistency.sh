#!/usr/bin/env bash
set -euo pipefail

API_URL="${API_URL:-http://localhost:8080}"
DB_CONTAINER="${DB_CONTAINER:-vipertrade-postgres}"
DB_USER="${DB_USER:-viper}"
DB_NAME="${DB_NAME:-vipertrade}"

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "ERROR: required command not found: $1" >&2
    exit 1
  }
}

need_cmd curl
need_cmd podman
need_cmd python3

PERF_JSON=$(curl -fsS "${API_URL}/api/v1/performance")

extract_window_bound() {
  local key="$1"
  local field="$2"
  python3 -c "import json,sys; print(json.loads(sys.stdin.read())[\"$key\"][\"$field\"])" <<<"${PERF_JSON}"
}

fetch_db_window() {
  local start_utc="$1"
  local end_utc="$2"

  podman exec -i "${DB_CONTAINER}" psql -U "${DB_USER}" -d "${DB_NAME}" -At -F '|' -c \
    "SELECT \
       COUNT(*)::bigint, \
       COUNT(*) FILTER (WHERE COALESCE(pnl,0) > 0)::bigint, \
       COALESCE(SUM(COALESCE(pnl,0))::double precision,0) \
     FROM trades \
     WHERE status='closed' \
       AND closed_at IS NOT NULL \
       AND closed_at >= '${start_utc}'::timestamptz \
       AND closed_at < '${end_utc}'::timestamptz;"
}

DB_24=$(fetch_db_window "$(extract_window_bound last_24h window_start_utc)" "$(extract_window_bound last_24h window_end_utc)")
DB_7D=$(fetch_db_window "$(extract_window_bound last_7d window_start_utc)" "$(extract_window_bound last_7d window_end_utc)")
DB_30D=$(fetch_db_window "$(extract_window_bound last_30d window_start_utc)" "$(extract_window_bound last_30d window_end_utc)")

python3 - <<'PY' "${PERF_JSON}" "${DB_24}" "${DB_7D}" "${DB_30D}"
import json
import math
import sys

api = json.loads(sys.argv[1])
db_rows = {
    "last_24h": sys.argv[2],
    "last_7d": sys.argv[3],
    "last_30d": sys.argv[4],
}

ok = True

for key, row in db_rows.items():
    db_total, db_wins, db_pnl = row.strip().split("|")
    db_total = int(db_total)
    db_wins = int(db_wins)
    db_pnl = float(db_pnl)

    api_window = api[key]
    api_total = int(api_window["total_trades"])
    api_wins = int(api_window["winning_trades"])
    api_pnl = float(api_window["total_pnl"])

    if api_total != db_total:
        ok = False
        print(f"MISMATCH {key}.total_trades api={api_total} db={db_total}")
    if api_wins != db_wins:
        ok = False
        print(f"MISMATCH {key}.winning_trades api={api_wins} db={db_wins}")
    if not math.isclose(api_pnl, round(db_pnl, 6), rel_tol=0.0, abs_tol=1e-6):
        ok = False
        print(f"MISMATCH {key}.total_pnl api={api_pnl} db={round(db_pnl, 6)}")

if ok:
    print("OK: API performance windows are consistent with DB aggregates")
    sys.exit(0)

sys.exit(1)
PY