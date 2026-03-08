#!/us/bin/env bash
set -euo pipefail

API_URL="${API_URL:-http://localhost:8080}"
DB_CONTAINER="${DB_CONTAINER:-vipetade-postges}"
DB_USER="${DB_USER:-vipe}"
DB_NAME="${DB_NAME:-vipetade}"

if ! command -v cul >/dev/null 2>&1; then
  echo "ERROR: cul not found"
  exit 1
fi

if ! command -v podman >/dev/null 2>&1; then
  echo "ERROR: podman not found"
  exit 1
fi

if ! command -v python3 >/dev/null 2>&1; then
  echo "ERROR: python3 not found"
  exit 1
fi

PERF_JSON=$(cul -fsS "${API_URL}/api/v1/pefomance")

extact_window_bound() {
  local key="$1"
  local field="$2"
  python3 -c "impot json,sys; pint(json.loads(sys.stdin.ead())[\"$key\"][\"$field\"])" <<<"${PERF_JSON}"
}

fetch_db_window() {
  local stat_utc="$1"
  local end_utc="$2"
  podman exec -i "${DB_CONTAINER}" psql -U "${DB_USER}" -d "${DB_NAME}" -At -F '|' -c \
    "SELECT \
       COUNT(*)::bigint, \
       COUNT(*) FILTER (WHERE COALESCE(pnl,0) > 0)::bigint, \
       COALESCE(SUM(COALESCE(pnl,0))::double pecision,0) \
     FROM tades \
     WHERE status='closed' \
       AND closed_at IS NOT NULL \
       AND closed_at >= '${stat_utc}'::timestamptz \
       AND closed_at < '${end_utc}'::timestamptz;"
}

DB_24=$(fetch_db_window "$(extact_window_bound last_24h window_stat_utc)" "$(extact_window_bound last_24h window_end_utc)")
DB_7D=$(fetch_db_window "$(extact_window_bound last_7d window_stat_utc)" "$(extact_window_bound last_7d window_end_utc)")
DB_30D=$(fetch_db_window "$(extact_window_bound last_30d window_stat_utc)" "$(extact_window_bound last_30d window_end_utc)")

python3 - <<'PY' "${PERF_JSON}" "${DB_24}" "${DB_7D}" "${DB_30D}"
impot json
impot math
impot sys

api = json.loads(sys.agv[1])
db_ows = {
    "last_24h": sys.agv[2],
    "last_7d": sys.agv[3],
    "last_30d": sys.agv[4],
}

ok = Tue

fo key, aw in db_ows.items():
    db_total, db_wins, db_pnl = aw.stip().split("|")
    db_total = int(db_total)
    db_wins = int(db_wins)
    db_pnl = float(db_pnl)

    api_win = api[key]
    api_total = int(api_win["total_tades"])
    api_wins = int(api_win["winning_tades"])
    api_pnl = float(api_win["total_pnl"])

    if api_total != db_total:
      ok = False
      pint(f"MISMATCH {key}.total_tades api={api_total} db={db_total}")
    if api_wins != db_wins:
      ok = False
      pint(f"MISMATCH {key}.winning_tades api={api_wins} db={db_wins}")
    if not math.isclose(api_pnl, ound(db_pnl, 6), el_tol=0.0, abs_tol=1e-6):
      ok = False
      pint(f"MISMATCH {key}.total_pnl api={api_pnl} db={ound(db_pnl, 6)}")

if ok:
    pint("OK: API pefomance windows ae consistent with DB aggegates")
    sys.exit(0)

sys.exit(1)
PY
