#!/usr/bin/env bash
set -euo pipefail

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
. "$ROOT_DIR/scripts/container-runtime.sh"
DATE_UTC="$(date -u +%Y-%m-%d)"
TS_UTC="$(date -u +%Y%m%dT%H%M%SZ)"
CREATED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
LOG_WINDOW="${LOG_WINDOW:-60m}"
DB_WINDOW_MINUTES="${DB_WINDOW_MINUTES:-60}"

ARTIFACT_DIR="$ROOT_DIR/docs/operations/artifacts/phase6"
JSON_FILE="$ARTIFACT_DIR/phase6_testnet_micro_${TS_UTC}.json"
MD_FILE="$ROOT_DIR/docs/operations/PHASE6_TESTNET_MICRO_${DATE_UTC}.md"

mkdir -p "$ARTIFACT_DIR"
cd "$ROOT_DIR"

if [[ -z "${OPERATOR_API_TOKEN:-}" && -f compose/.env ]]; then
  TOKEN_FROM_ENV_FILE="$(awk -F= '/^OPERATOR_API_TOKEN=/{print $2}' compose/.env | tail -n 1)"
  if [[ -n "${TOKEN_FROM_ENV_FILE:-}" ]]; then
    OPERATOR_API_TOKEN="$TOKEN_FROM_ENV_FILE"
    export OPERATOR_API_TOKEN
  fi
fi

echo -e "${GREEN}ViperTrade - Phase 6 Testnet Micro Gate${NC}"
echo "================================================"

ISSUES=0

if ./scripts/health-check.sh >/tmp/viper_phase6_testnet_health.log 2>&1; then
  HEALTH_OK=true
  echo -e "${GREEN}OK: health-check passed${NC}"
else
  HEALTH_OK=false
  echo -e "${RED}ERROR: health-check failed${NC}"
  tail -n 80 /tmp/viper_phase6_testnet_health.log || true
  ISSUES=$((ISSUES + 1))
fi

if ./scripts/check-api-metrics-consistency.sh >/tmp/viper_phase6_testnet_perf.log 2>&1; then
  PERF_OK=true
  echo -e "${GREEN}OK: API performance consistency passed${NC}"
else
  PERF_OK=false
  echo -e "${RED}ERROR: API performance consistency failed${NC}"
  tail -n 80 /tmp/viper_phase6_testnet_perf.log || true
  ISSUES=$((ISSUES + 1))
fi

ENV_BYBIT="$(awk -F= '/^BYBIT_ENV=/{print $2}' compose/.env | tail -n 1)"
ENV_LIVE="$(awk -F= '/^EXECUTOR_ENABLE_LIVE_ORDERS=/{print $2}' compose/.env | tail -n 1)"
RUNTIME_BYBIT="$(container_exec vipertrade-executor env | awk -F= '/^BYBIT_ENV=/{print $2}' | tail -n 1 || true)"
RUNTIME_LIVE="$(container_exec vipertrade-executor env | awk -F= '/^EXECUTOR_ENABLE_LIVE_ORDERS=/{print $2}' | tail -n 1 || true)"

if [[ "${ENV_BYBIT,,}" == "testnet" && "${RUNTIME_BYBIT,,}" == "testnet" ]]; then
  TESTNET_MODE_OK=true
  echo -e "${GREEN}OK: testnet mode confirmed (env/runtime)${NC}"
else
  TESTNET_MODE_OK=false
  echo -e "${RED}ERROR: expected testnet mode but got env=${ENV_BYBIT} runtime=${RUNTIME_BYBIT}${NC}"
  ISSUES=$((ISSUES + 1))
fi

if [[ "${ENV_LIVE,,}" == "false" && "${RUNTIME_LIVE,,}" == "false" ]]; then
  SAFE_LIVE_OK=true
  echo -e "${GREEN}OK: safe live posture confirmed (live orders disabled)${NC}"
else
  SAFE_LIVE_OK=false
  echo -e "${RED}ERROR: live orders must stay disabled env=${ENV_LIVE} runtime=${RUNTIME_LIVE}${NC}"
  ISSUES=$((ISSUES + 1))
fi

ROLLBACK_TESTED=false
ROLLBACK_OK=false
if [[ -n "${OPERATOR_API_TOKEN:-}" ]]; then
  ROLLBACK_TESTED=true
  if REASON='phase6_testnet_gate_enable' ./scripts/kill-switch-control.sh enable >/tmp/viper_phase6_testnet_kill_enable.log 2>&1 && \
     REASON='phase6_testnet_gate_disable' ./scripts/kill-switch-control.sh disable >/tmp/viper_phase6_testnet_kill_disable.log 2>&1; then
    ROLLBACK_OK=true
    echo -e "${GREEN}OK: kill-switch rollback flow passed${NC}"
  else
    echo -e "${RED}ERROR: kill-switch rollback flow failed${NC}"
    tail -n 80 /tmp/viper_phase6_testnet_kill_enable.log 2>/dev/null || true
    tail -n 80 /tmp/viper_phase6_testnet_kill_disable.log 2>/dev/null || true
    ISSUES=$((ISSUES + 1))
  fi
else
  echo -e "${RED}ERROR: OPERATOR_API_TOKEN not configured${NC}"
  ISSUES=$((ISSUES + 1))
fi

CRITICAL_RECON=$(container_exec_i vipertrade-postgres psql -U "${POSTGRES_USER:-viper}" -d "${POSTGRES_DB:-vipertrade}" -At -c \
  "SELECT COUNT(*)::bigint FROM system_events WHERE event_type='reconciliation_cycle' AND severity='critical' AND timestamp >= NOW() - INTERVAL '${DB_WINDOW_MINUTES} minutes';" 2>/dev/null || echo 999)

if [[ "$CRITICAL_RECON" =~ ^[0-9]+$ ]] && (( CRITICAL_RECON == 0 )); then
  RECON_OK=true
  echo -e "${GREEN}OK: no critical reconciliation events in ${DB_WINDOW_MINUTES}m${NC}"
else
  RECON_OK=false
  echo -e "${RED}ERROR: critical reconciliation events found=${CRITICAL_RECON}${NC}"
  ISSUES=$((ISSUES + 1))
fi

CLOSE_ERRORS=$(container_logs --since "$LOG_WINDOW" vipertrade-executor 2>&1 | grep -Eci 'submitted_close_no_persist|close_reconcile_failed|close request rejected' || true)
if [[ "$CLOSE_ERRORS" =~ ^[0-9]+$ ]] && (( CLOSE_ERRORS == 0 )); then
  CLOSE_OK=true
  echo -e "${GREEN}OK: no executor close-path errors in ${LOG_WINDOW}${NC}"
else
  CLOSE_OK=false
  echo -e "${RED}ERROR: executor close-path error count=${CLOSE_ERRORS}${NC}"
  ISSUES=$((ISSUES + 1))
fi

STATUS='passed'
DECISION='GO_TESTNET_READY'
if (( ISSUES > 0 )); then
  STATUS='failed'
  DECISION='HOLD'
fi

cat > "$JSON_FILE" <<JSON
{
  "schema_version": "v1",
  "created_at_utc": "$CREATED_AT",
  "status": "$STATUS",
  "decision": "$DECISION",
  "window": {
    "log_window": "$LOG_WINDOW",
    "db_window_minutes": $DB_WINDOW_MINUTES
  },
  "checks": {
    "health_check": $HEALTH_OK,
    "api_performance_consistency": $PERF_OK,
    "testnet_mode_ok": $TESTNET_MODE_OK,
    "safe_live_disabled_ok": $SAFE_LIVE_OK,
    "rollback_tested": $ROLLBACK_TESTED,
    "rollback_ok": $ROLLBACK_OK,
    "reconciliation_gate": $RECON_OK,
    "close_path_errors": $CLOSE_OK
  },
  "signals": {
    "critical_reconciliation_events": $CRITICAL_RECON,
    "executor_close_error_count": $CLOSE_ERRORS
  },
  "issues": $ISSUES
}
JSON

cat > "$MD_FILE" <<MD
# Phase 6 Testnet Micro Gate - ${DATE_UTC}

## Decision

- decision: ${DECISION}
- status: ${STATUS}
- issues: ${ISSUES}

## Checks

- health_check: ${HEALTH_OK}
- api_performance_consistency: ${PERF_OK}
- testnet_mode_ok: ${TESTNET_MODE_OK}
- safe_live_disabled_ok: ${SAFE_LIVE_OK}
- rollback_tested: ${ROLLBACK_TESTED}
- rollback_ok: ${ROLLBACK_OK}
- reconciliation_gate: ${RECON_OK}
- close_path_errors: ${CLOSE_OK}

## Signals

- critical_reconciliation_events(${DB_WINDOW_MINUTES}m): ${CRITICAL_RECON}
- executor_close_error_count(${LOG_WINDOW}): ${CLOSE_ERRORS}

## Artifact

- ${JSON_FILE#$ROOT_DIR/}
MD

echo "Evidence JSON: ${JSON_FILE#$ROOT_DIR/}"
echo "Evidence MD:   ${MD_FILE#$ROOT_DIR/}"

if (( ISSUES == 0 )); then
  echo -e "${GREEN}SUCCESS: Phase 6 testnet micro gate passed${NC}"
  exit 0
fi

echo -e "${YELLOW}WARN: Phase 6 testnet micro gate found ${ISSUES} issue(s)${NC}"
exit 1
