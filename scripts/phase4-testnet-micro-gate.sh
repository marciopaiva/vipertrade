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
LOG_WINDOW="${LOG_WINDOW:-60m}"
DB_WINDOW_MINUTES="${DB_WINDOW_MINUTES:-60}"

ARTIFACT_DIR="$ROOT_DIR/docs/operations/artifacts/testnet"
JSON_FILE="$ARTIFACT_DIR/testnet_micro_${TS_UTC}.json"
MD_FILE="$ROOT_DIR/docs/operations/PHASE4_TESTNET_MICRO_${DATE_UTC}.md"

mkdir -p "$ARTIFACT_DIR"
cd "$ROOT_DIR"

echo -e "${GREEN}ViperTrade - Phase 4 Testnet Micro Gate${NC}"
echo "================================================"

ISSUES=0

if ./scripts/health-check.sh >/tmp/viper_phase4_testnet_health.log 2>&1; then
  HEALTH_OK=true
  echo -e "${GREEN}OK: health-check passed${NC}"
else
  HEALTH_OK=false
  echo -e "${RED}ERROR: health-check failed${NC}"
  tail -n 80 /tmp/viper_phase4_testnet_health.log || true
  ISSUES=$((ISSUES + 1))
fi

if ./scripts/check-api-metrics-consistency.sh >/tmp/viper_phase4_testnet_perf.log 2>&1; then
  PERF_OK=true
  echo -e "${GREEN}OK: API performance consistency passed${NC}"
else
  PERF_OK=false
  echo -e "${RED}ERROR: API performance consistency failed${NC}"
  tail -n 80 /tmp/viper_phase4_testnet_perf.log || true
  ISSUES=$((ISSUES + 1))
fi

CRITICAL_RECON=$(podman exec -i vipertrade-postgres psql -U "${POSTGRES_USER:-viper}" -d "${POSTGRES_DB:-vipertrade}" -At -c \
  "SELECT COUNT(*)::bigint FROM system_events WHERE event_type='reconciliation_cycle' AND severity='critical' AND timestamp >= NOW() - INTERVAL '${DB_WINDOW_MINUTES} minutes';" 2>/dev/null || echo 999)

FAILED_RECON_FIX=$(podman exec -i vipertrade-postgres psql -U "${POSTGRES_USER:-viper}" -d "${POSTGRES_DB:-vipertrade}" -At -c \
  "SELECT COUNT(*)::bigint FROM system_events WHERE event_type='reconciliation_fix_result' AND COALESCE(data->>'status','')='failed' AND timestamp >= NOW() - INTERVAL '${DB_WINDOW_MINUTES} minutes';" 2>/dev/null || echo 999)

if [[ "$CRITICAL_RECON" =~ ^[0-9]+$ ]] && [[ "$FAILED_RECON_FIX" =~ ^[0-9]+$ ]] && (( CRITICAL_RECON == 0 )) && (( FAILED_RECON_FIX == 0 )); then
  RECON_OK=true
  echo -e "${GREEN}OK: reconciliation critical/fail counts are zero (${DB_WINDOW_MINUTES}m)${NC}"
else
  RECON_OK=false
  echo -e "${RED}ERROR: reconciliation has critical/failure signals critical=${CRITICAL_RECON} failed_fix=${FAILED_RECON_FIX}${NC}"
  ISSUES=$((ISSUES + 1))
fi

CLOSE_ERRORS=$(podman logs --since "$LOG_WINDOW" vipertrade-executor 2>&1 | grep -Eci 'submitted_close_no_persist|close_reconcile_failed|close request rejected' || true)
if [[ "$CLOSE_ERRORS" =~ ^[0-9]+$ ]] && (( CLOSE_ERRORS == 0 )); then
  CLOSE_OK=true
  echo -e "${GREEN}OK: executor close-path errors not found in last ${LOG_WINDOW}${NC}"
else
  CLOSE_OK=false
  echo -e "${RED}ERROR: executor close-path error patterns found count=${CLOSE_ERRORS}${NC}"
  ISSUES=$((ISSUES + 1))
fi

KILL_SWITCH_TESTED=false
KILL_SWITCH_OK=false

if [[ -n "${OPERATOR_API_TOKEN:-}" ]]; then
  KILL_SWITCH_TESTED=true
  if REASON='phase4_testnet_gate_enable' ./scripts/kill-switch-control.sh enable >/tmp/viper_phase4_kill_enable.log 2>&1 && \
     REASON='phase4_testnet_gate_disable' ./scripts/kill-switch-control.sh disable >/tmp/viper_phase4_kill_disable.log 2>&1; then
    KILL_SWITCH_OK=true
    echo -e "${GREEN}OK: kill-switch enable/disable flow passed${NC}"
  else
    KILL_SWITCH_OK=false
    echo -e "${RED}ERROR: kill-switch enable/disable flow failed${NC}"
    tail -n 60 /tmp/viper_phase4_kill_enable.log 2>/dev/null || true
    tail -n 60 /tmp/viper_phase4_kill_disable.log 2>/dev/null || true
    ISSUES=$((ISSUES + 1))
  fi
else
  echo -e "${YELLOW}WARN: OPERATOR_API_TOKEN not set; kill-switch active toggle test skipped${NC}"
fi

STATUS='passed'
if (( ISSUES > 0 )); then
  STATUS='failed'
fi

cat > "$JSON_FILE" <<JSON
{
  "schema_version": "v1",
  "created_at_utc": "$CREATED_AT",
  "status": "$STATUS",
  "window": {
    "log_window": "$LOG_WINDOW",
    "db_window_minutes": $DB_WINDOW_MINUTES
  },
  "checks": {
    "health_check": $HEALTH_OK,
    "api_performance_consistency": $PERF_OK,
    "reconciliation_gate": $RECON_OK,
    "close_path_errors": $CLOSE_OK,
    "kill_switch_tested": $KILL_SWITCH_TESTED,
    "kill_switch_ok": $KILL_SWITCH_OK
  },
  "signals": {
    "critical_reconciliation_events": $CRITICAL_RECON,
    "failed_reconciliation_fixes": $FAILED_RECON_FIX,
    "executor_close_error_count": $CLOSE_ERRORS
  },
  "issues": $ISSUES
}
JSON

cat > "$MD_FILE" <<MD
# Phase 4 Testnet Micro Gate - ${DATE_UTC}

## Summary

- status: ${STATUS}
- issues: ${ISSUES}
- health_check: ${HEALTH_OK}
- api_performance_consistency: ${PERF_OK}
- reconciliation_gate: ${RECON_OK}
- close_path_errors: ${CLOSE_OK}
- kill_switch_tested: ${KILL_SWITCH_TESTED}
- kill_switch_ok: ${KILL_SWITCH_OK}
- critical_reconciliation_events(${DB_WINDOW_MINUTES}m): ${CRITICAL_RECON}
- failed_reconciliation_fixes(${DB_WINDOW_MINUTES}m): ${FAILED_RECON_FIX}
- executor_close_error_count(${LOG_WINDOW}): ${CLOSE_ERRORS}

## Artifact

- ${JSON_FILE#$ROOT_DIR/}
MD

echo "Evidence JSON: ${JSON_FILE#$ROOT_DIR/}"
echo "Evidence MD:   ${MD_FILE#$ROOT_DIR/}"

if (( ISSUES == 0 )); then
  echo -e "${GREEN}SUCCESS: Phase 4 testnet micro gate passed${NC}"
  exit 0
fi

echo -e "${YELLOW}WARN: Phase 4 testnet micro gate found ${ISSUES} issue(s)${NC}"
exit 1