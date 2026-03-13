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
MAX_ROLLBACK_SEC="${MAX_ROLLBACK_SEC:-30}"

ARTIFACT_DIR="$ROOT_DIR/docs/operations/artifacts/phase6"
JSON_FILE="$ARTIFACT_DIR/phase6_baseline_${TS_UTC}.json"
MD_FILE="$ROOT_DIR/docs/operations/PHASE6_BASELINE_${DATE_UTC}.md"
BACKUP_FILE="/tmp/viper_phase6_schema_${TS_UTC}.sql"

mkdir -p "$ARTIFACT_DIR"
cd "$ROOT_DIR"

# Fallback to compose/.env when token is not exported in shell env.
if [[ -z "${OPERATOR_API_TOKEN:-}" && -f compose/.env ]]; then
  TOKEN_FROM_ENV_FILE="$(awk -F= '/^OPERATOR_API_TOKEN=/{print $2}' compose/.env | tail -n 1)"
  if [[ -n "${TOKEN_FROM_ENV_FILE:-}" ]]; then
    OPERATOR_API_TOKEN="$TOKEN_FROM_ENV_FILE"
    export OPERATOR_API_TOKEN
  fi
fi

echo -e "${GREEN}ViperTrade - Phase 6 Mainnet Readiness Baseline${NC}"
echo "================================================"

ISSUES=0

if ./scripts/security-check.sh >/tmp/viper_phase6_security.log 2>&1; then
  SECURITY_OK=true
  echo -e "${GREEN}OK: security-check passed${NC}"
else
  SECURITY_OK=false
  echo -e "${RED}ERROR: security-check failed${NC}"
  tail -n 100 /tmp/viper_phase6_security.log || true
  ISSUES=$((ISSUES + 1))
fi

if ./scripts/health-check.sh >/tmp/viper_phase6_health.log 2>&1; then
  HEALTH_OK=true
  echo -e "${GREEN}OK: health-check passed${NC}"
else
  HEALTH_OK=false
  echo -e "${RED}ERROR: health-check failed${NC}"
  tail -n 100 /tmp/viper_phase6_health.log || true
  ISSUES=$((ISSUES + 1))
fi

if ./scripts/check-api-metrics-consistency.sh >/tmp/viper_phase6_perf.log 2>&1; then
  PERF_OK=true
  echo -e "${GREEN}OK: API performance consistency passed${NC}"
else
  PERF_OK=false
  echo -e "${RED}ERROR: API performance consistency failed${NC}"
  tail -n 100 /tmp/viper_phase6_perf.log || true
  ISSUES=$((ISSUES + 1))
fi

ENV_LIVE_FLAG="$(awk -F= '/^EXECUTOR_ENABLE_LIVE_ORDERS=/{print $2}' compose/.env | tail -n 1)"
if [[ "${ENV_LIVE_FLAG,,}" == "false" || "${ENV_LIVE_FLAG}" == "0" ]]; then
  SAFE_ENV_OK=true
  echo -e "${GREEN}OK: compose/.env safe posture EXECUTOR_ENABLE_LIVE_ORDERS=${ENV_LIVE_FLAG}${NC}"
else
  SAFE_ENV_OK=false
  echo -e "${RED}ERROR: compose/.env unsafe posture EXECUTOR_ENABLE_LIVE_ORDERS=${ENV_LIVE_FLAG}${NC}"
  ISSUES=$((ISSUES + 1))
fi

RUNTIME_LIVE_FLAG="$(container_exec vipertrade-executor env | awk -F= '/^EXECUTOR_ENABLE_LIVE_ORDERS=/{print $2}' | tail -n 1 || true)"
if [[ "${RUNTIME_LIVE_FLAG,,}" == "false" || "${RUNTIME_LIVE_FLAG}" == "0" ]]; then
  SAFE_RUNTIME_OK=true
  echo -e "${GREEN}OK: runtime safe posture EXECUTOR_ENABLE_LIVE_ORDERS=${RUNTIME_LIVE_FLAG}${NC}"
else
  SAFE_RUNTIME_OK=false
  echo -e "${RED}ERROR: runtime unsafe posture EXECUTOR_ENABLE_LIVE_ORDERS=${RUNTIME_LIVE_FLAG}${NC}"
  ISSUES=$((ISSUES + 1))
fi

ROLLBACK_TESTED=false
ROLLBACK_OK=false
ROLLBACK_ELAPSED_SEC=-1
if [[ -n "${OPERATOR_API_TOKEN:-}" ]]; then
  ROLLBACK_TESTED=true
  START_TS="$(date +%s)"
  if REASON='phase6_readiness_enable' ./scripts/kill-switch-control.sh enable >/tmp/viper_phase6_kill_enable.log 2>&1 && \
     REASON='phase6_readiness_disable' ./scripts/kill-switch-control.sh disable >/tmp/viper_phase6_kill_disable.log 2>&1; then
    END_TS="$(date +%s)"
    ROLLBACK_ELAPSED_SEC="$((END_TS - START_TS))"
    if (( ROLLBACK_ELAPSED_SEC <= MAX_ROLLBACK_SEC )); then
      ROLLBACK_OK=true
      echo -e "${GREEN}OK: rollback drill passed (${ROLLBACK_ELAPSED_SEC}s <= ${MAX_ROLLBACK_SEC}s)${NC}"
    else
      echo -e "${RED}ERROR: rollback drill too slow (${ROLLBACK_ELAPSED_SEC}s > ${MAX_ROLLBACK_SEC}s)${NC}"
      ISSUES=$((ISSUES + 1))
    fi
  else
    echo -e "${RED}ERROR: rollback drill failed${NC}"
    tail -n 80 /tmp/viper_phase6_kill_enable.log 2>/dev/null || true
    tail -n 80 /tmp/viper_phase6_kill_disable.log 2>/dev/null || true
    ISSUES=$((ISSUES + 1))
  fi
else
  echo -e "${RED}ERROR: OPERATOR_API_TOKEN not configured for rollback drill${NC}"
  ISSUES=$((ISSUES + 1))
fi

DR_BACKUP_OK=false
DR_BACKUP_BYTES=0
DR_BACKUP_SHA256=""
if container_exec_i vipertrade-postgres pg_dump -U "${POSTGRES_USER:-viper}" -d "${POSTGRES_DB:-vipertrade}" -s >"${BACKUP_FILE}" 2>/tmp/viper_phase6_backup.log; then
  if [[ -s "${BACKUP_FILE}" ]]; then
    DR_BACKUP_OK=true
    DR_BACKUP_BYTES="$(wc -c < "${BACKUP_FILE}")"
    DR_BACKUP_SHA256="$(sha256sum "${BACKUP_FILE}" | awk '{print $1}')"
    echo -e "${GREEN}OK: DR schema backup drill passed (${DR_BACKUP_BYTES} bytes)${NC}"
  else
    echo -e "${RED}ERROR: DR schema backup output is empty${NC}"
    ISSUES=$((ISSUES + 1))
  fi
else
  echo -e "${RED}ERROR: DR schema backup drill failed${NC}"
  tail -n 100 /tmp/viper_phase6_backup.log 2>/dev/null || true
  ISSUES=$((ISSUES + 1))
fi

STATUS="passed"
if (( ISSUES > 0 )); then
  STATUS="failed"
fi

cat > "$JSON_FILE" <<JSON
{
  "schema_version": "v1",
  "created_at_utc": "$CREATED_AT",
  "status": "$STATUS",
  "thresholds": {
    "max_rollback_sec": $MAX_ROLLBACK_SEC
  },
  "checks": {
    "security_check": $SECURITY_OK,
    "health_check": $HEALTH_OK,
    "api_performance_consistency": $PERF_OK,
    "safe_env_live_disabled": $SAFE_ENV_OK,
    "safe_runtime_live_disabled": $SAFE_RUNTIME_OK,
    "rollback_tested": $ROLLBACK_TESTED,
    "rollback_ok": $ROLLBACK_OK,
    "dr_backup_ok": $DR_BACKUP_OK
  },
  "signals": {
    "rollback_elapsed_sec": $ROLLBACK_ELAPSED_SEC,
    "dr_backup_file": "$BACKUP_FILE",
    "dr_backup_bytes": $DR_BACKUP_BYTES,
    "dr_backup_sha256": "$DR_BACKUP_SHA256"
  },
  "issues": $ISSUES
}
JSON

cat > "$MD_FILE" <<MD
# Phase 6 Baseline Validation - ${DATE_UTC}

## Summary

- status: ${STATUS}
- issues: ${ISSUES}
- security_check: ${SECURITY_OK}
- health_check: ${HEALTH_OK}
- api_performance_consistency: ${PERF_OK}
- safe_env_live_disabled: ${SAFE_ENV_OK}
- safe_runtime_live_disabled: ${SAFE_RUNTIME_OK}
- rollback_tested: ${ROLLBACK_TESTED}
- rollback_ok: ${ROLLBACK_OK}
- rollback_elapsed_sec: ${ROLLBACK_ELAPSED_SEC}
- dr_backup_ok: ${DR_BACKUP_OK}
- dr_backup_bytes: ${DR_BACKUP_BYTES}
- dr_backup_sha256: ${DR_BACKUP_SHA256}

## Artifact

- ${JSON_FILE#$ROOT_DIR/}
MD

echo "Evidence JSON: ${JSON_FILE#$ROOT_DIR/}"
echo "Evidence MD:   ${MD_FILE#$ROOT_DIR/}"

if (( ISSUES == 0 )); then
  echo -e "${GREEN}SUCCESS: Phase 6 readiness baseline passed${NC}"
  exit 0
fi

echo -e "${YELLOW}WARN: Phase 6 readiness baseline found ${ISSUES} issue(s)${NC}"
exit 1
