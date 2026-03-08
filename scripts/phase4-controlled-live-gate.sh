#!/usr/bin/env bash
set -euo pipefail

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
DATE_UTC="$(date -u +%Y-%m-%d)"
TS_UTC="$(date -u +%Y%m%dT%H%M%SZ)"
CREATED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

WINDOW_HOURS="${WINDOW_HOURS:-24}"
MAX_DRAWDOWN_PCT="${MAX_DRAWDOWN_PCT:-10}"
MAX_CRITICAL_EVENTS="${MAX_CRITICAL_EVENTS:-3}"
MIN_CLOSED_TRADES="${MIN_CLOSED_TRADES:-1}"

ARTIFACT_DIR="$ROOT_DIR/docs/operations/artifacts/live"
JSON_FILE="$ARTIFACT_DIR/controlled_live_${TS_UTC}.json"
MD_FILE="$ROOT_DIR/docs/operations/PHASE4_CONTROLLED_LIVE_${DATE_UTC}.md"

mkdir -p "$ARTIFACT_DIR"
cd "$ROOT_DIR"

# Fallback to compose/.env when the token is not exported in shell env.
if [[ -z "${OPERATOR_API_TOKEN:-}" && -f compose/.env ]]; then
  TOKEN_FROM_ENV_FILE="$(awk -F= '/^OPERATOR_API_TOKEN=/{print $2}' compose/.env | tail -n 1)"
  if [[ -n "${TOKEN_FROM_ENV_FILE:-}" ]]; then
    OPERATOR_API_TOKEN="$TOKEN_FROM_ENV_FILE"
    export OPERATOR_API_TOKEN
  fi
fi

echo -e "${GREEN}ViperTrade - Phase 4 Controlled Live Gate${NC}"
echo "================================================"

ISSUES=0

if ./scripts/health-check.sh >/tmp/viper_phase4_live_health.log 2>&1; then
  HEALTH_OK=true
  echo -e "${GREEN}OK: health-check passed${NC}"
else
  HEALTH_OK=false
  echo -e "${RED}ERROR: health-check failed${NC}"
  tail -n 80 /tmp/viper_phase4_live_health.log || true
  ISSUES=$((ISSUES + 1))
fi

if ./scripts/check-api-metrics-consistency.sh >/tmp/viper_phase4_live_perf.log 2>&1; then
  PERF_OK=true
  echo -e "${GREEN}OK: API performance consistency passed${NC}"
else
  PERF_OK=false
  echo -e "${RED}ERROR: API performance consistency failed${NC}"
  tail -n 80 /tmp/viper_phase4_live_perf.log || true
  ISSUES=$((ISSUES + 1))
fi

PERF_JSON=$(curl -fsS http://localhost:8080/api/v1/performance || echo '{}')
STATUS_JSON=$(curl -fsS http://localhost:8080/api/v1/status || echo '{}')

MAX_DRAWDOWN_30D=$(python3 - <<'PY' "$PERF_JSON"
import json,sys
try:
    d=json.loads(sys.argv[1])
    print(float(d.get('max_drawdown_30d', 0.0)))
except Exception:
    print('999')
PY
)

if python3 - <<'PY' "$MAX_DRAWDOWN_30D" "$MAX_DRAWDOWN_PCT"
import sys
md=float(sys.argv[1]); limit=float(sys.argv[2])
sys.exit(0 if md <= limit else 1)
PY
then
  DRAWDOWN_OK=true
  echo -e "${GREEN}OK: max_drawdown_30d=${MAX_DRAWDOWN_30D}% (limit=${MAX_DRAWDOWN_PCT}%)${NC}"
else
  DRAWDOWN_OK=false
  echo -e "${RED}ERROR: max_drawdown_30d=${MAX_DRAWDOWN_30D}% exceeds limit=${MAX_DRAWDOWN_PCT}%${NC}"
  ISSUES=$((ISSUES + 1))
fi

CRITICAL_EVENTS=$(podman exec -i vipertrade-postgres psql -U "${POSTGRES_USER:-viper}" -d "${POSTGRES_DB:-vipertrade}" -At -c \
  "SELECT COUNT(*)::bigint FROM system_events WHERE severity='critical' AND timestamp >= NOW() - INTERVAL '${WINDOW_HOURS} hours';" 2>/dev/null || echo 999)

if [[ "$CRITICAL_EVENTS" =~ ^[0-9]+$ ]] && (( CRITICAL_EVENTS <= MAX_CRITICAL_EVENTS )); then
  ALERT_STORM_OK=true
  echo -e "${GREEN}OK: critical events=${CRITICAL_EVENTS} in last ${WINDOW_HOURS}h (limit=${MAX_CRITICAL_EVENTS})${NC}"
else
  ALERT_STORM_OK=false
  echo -e "${RED}ERROR: critical events=${CRITICAL_EVENTS} exceed limit=${MAX_CRITICAL_EVENTS}${NC}"
  ISSUES=$((ISSUES + 1))
fi

CLOSED_TRADES_WINDOW=$(podman exec -i vipertrade-postgres psql -U "${POSTGRES_USER:-viper}" -d "${POSTGRES_DB:-vipertrade}" -At -c \
  "SELECT COUNT(*)::bigint FROM trades WHERE status='closed' AND closed_at >= NOW() - INTERVAL '${WINDOW_HOURS} hours';" 2>/dev/null || echo -1)

if [[ "$CLOSED_TRADES_WINDOW" =~ ^[0-9]+$ ]] && (( CLOSED_TRADES_WINDOW >= MIN_CLOSED_TRADES )); then
  ACTIVITY_OK=true
  echo -e "${GREEN}OK: closed trades in ${WINDOW_HOURS}h=${CLOSED_TRADES_WINDOW} (min=${MIN_CLOSED_TRADES})${NC}"
else
  ACTIVITY_OK=false
  echo -e "${RED}ERROR: insufficient live activity in ${WINDOW_HOURS}h closed_trades=${CLOSED_TRADES_WINDOW} (min=${MIN_CLOSED_TRADES})${NC}"
  ISSUES=$((ISSUES + 1))
fi

OP_CONTROLS=$(python3 - <<'PY' "$STATUS_JSON"
import json,sys
try:
    d=json.loads(sys.argv[1])
    print('true' if d.get('operator_controls_enabled') else 'false')
except Exception:
    print('false')
PY
)

ROLLBACK_TESTED=false
ROLLBACK_OK=false
if [[ "$OP_CONTROLS" == "true" && -n "${OPERATOR_API_TOKEN:-}" ]]; then
  ROLLBACK_TESTED=true
  if REASON='phase4_live_gate_enable' ./scripts/kill-switch-control.sh enable >/tmp/viper_phase4_live_kill_enable.log 2>&1 && \
     REASON='phase4_live_gate_disable' ./scripts/kill-switch-control.sh disable >/tmp/viper_phase4_live_kill_disable.log 2>&1; then
    ROLLBACK_OK=true
    echo -e "${GREEN}OK: rollback path (kill-switch toggle) validated${NC}"
  else
    echo -e "${RED}ERROR: rollback path test failed${NC}"
    tail -n 40 /tmp/viper_phase4_live_kill_enable.log 2>/dev/null || true
    tail -n 40 /tmp/viper_phase4_live_kill_disable.log 2>/dev/null || true
    ISSUES=$((ISSUES + 1))
  fi
else
  echo -e "${YELLOW}WARN: rollback path test skipped (operator controls/token unavailable)${NC}"
  ISSUES=$((ISSUES + 1))
fi

DECISION='GO'
if (( ISSUES > 0 )); then
  DECISION='HOLD'
fi

cat > "$JSON_FILE" <<JSON
{
  "schema_version": "v1",
  "created_at_utc": "$CREATED_AT",
  "decision": "$DECISION",
  "window_hours": $WINDOW_HOURS,
  "limits": {
    "max_drawdown_pct": $MAX_DRAWDOWN_PCT,
    "max_critical_events": $MAX_CRITICAL_EVENTS,
    "min_closed_trades": $MIN_CLOSED_TRADES
  },
  "checks": {
    "health_check": $HEALTH_OK,
    "api_performance_consistency": $PERF_OK,
    "drawdown_gate": $DRAWDOWN_OK,
    "alert_storm_gate": $ALERT_STORM_OK,
    "activity_gate": $ACTIVITY_OK,
    "rollback_tested": $ROLLBACK_TESTED,
    "rollback_ok": $ROLLBACK_OK
  },
  "signals": {
    "max_drawdown_30d": $MAX_DRAWDOWN_30D,
    "critical_events_window": $CRITICAL_EVENTS,
    "closed_trades_window": $CLOSED_TRADES_WINDOW,
    "operator_controls_enabled": $OP_CONTROLS
  },
  "issues": $ISSUES
}
JSON

cat > "$MD_FILE" <<MD
# Phase 4 Controlled Live Gate - ${DATE_UTC}

## Decision

- decision: ${DECISION}
- issues: ${ISSUES}

## Checks

- health_check: ${HEALTH_OK}
- api_performance_consistency: ${PERF_OK}
- drawdown_gate: ${DRAWDOWN_OK}
- alert_storm_gate: ${ALERT_STORM_OK}
- activity_gate: ${ACTIVITY_OK}
- rollback_tested: ${ROLLBACK_TESTED}
- rollback_ok: ${ROLLBACK_OK}

## Signals

- max_drawdown_30d: ${MAX_DRAWDOWN_30D}
- critical_events_${WINDOW_HOURS}h: ${CRITICAL_EVENTS}
- closed_trades_${WINDOW_HOURS}h: ${CLOSED_TRADES_WINDOW}
- operator_controls_enabled: ${OP_CONTROLS}

## Artifact

- ${JSON_FILE#$ROOT_DIR/}
MD

echo "Evidence JSON: ${JSON_FILE#$ROOT_DIR/}"
echo "Evidence MD:   ${MD_FILE#$ROOT_DIR/}"

if [[ "$DECISION" == "GO" ]]; then
  echo -e "${GREEN}SUCCESS: Controlled live gate decision=GO${NC}"
  exit 0
fi

echo -e "${YELLOW}WARN: Controlled live gate decision=HOLD${NC}"
exit 1
