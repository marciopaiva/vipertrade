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

ARTIFACT_DIR="$ROOT_DIR/docs/operations/artifacts/paper"
JSON_FILE="$ARTIFACT_DIR/paper_regression_${TS_UTC}.json"
MD_FILE="$ROOT_DIR/docs/operations/PHASE4_PAPER_REGRESSION_${DATE_UTC}.md"

mkdir -p "$ARTIFACT_DIR"
cd "$ROOT_DIR"

echo -e "${GREEN}ViperTrade - Phase 4 Paper Regression${NC}"
echo "================================================"

ISSUES=0

if ./scripts/health-check.sh >/tmp/viper_phase4_paper_health.log 2>&1; then
  HEALTH_OK=true
  echo -e "${GREEN}OK: health-check passed${NC}"
else
  HEALTH_OK=false
  echo -e "${RED}ERROR: health-check failed${NC}"
  tail -n 80 /tmp/viper_phase4_paper_health.log || true
  ISSUES=$((ISSUES + 1))
fi

if ./scripts/check-api-metrics-consistency.sh >/tmp/viper_phase4_paper_consistency.log 2>&1; then
  PERF_OK=true
  echo -e "${GREEN}OK: API performance consistency passed${NC}"
else
  PERF_OK=false
  echo -e "${RED}ERROR: API performance consistency failed${NC}"
  tail -n 80 /tmp/viper_phase4_paper_consistency.log || true
  ISSUES=$((ISSUES + 1))
fi

nums=$(podman exec vipertrade-redis redis-cli PUBSUB NUMSUB viper:market_data viper:decisions 2>/dev/null || true)
MD_SUB=$(echo "$nums" | awk 'NR==2 {print $1}')
DEC_SUB=$(echo "$nums" | awk 'NR==4 {print $1}')

if [[ -z "${MD_SUB:-}" ]]; then MD_SUB=0; fi
if [[ -z "${DEC_SUB:-}" ]]; then DEC_SUB=0; fi

if (( MD_SUB < 1 || DEC_SUB < 1 )); then
  REDIS_OK=false
  echo -e "${YELLOW}WARN: Redis subscribers low market_data=${MD_SUB} decisions=${DEC_SUB}${NC}"
  ISSUES=$((ISSUES + 1))
else
  REDIS_OK=true
  echo -e "${GREEN}OK: Redis subscribers market_data=${MD_SUB} decisions=${DEC_SUB}${NC}"
fi

DB_COUNTS=$(podman exec -i vipertrade-postgres psql -U "${POSTGRES_USER:-viper}" -d "${POSTGRES_DB:-vipertrade}" -At -F '|' -c "SELECT
  (SELECT COUNT(*) FROM positions WHERE status = 'open')::bigint,
  (SELECT COUNT(*) FROM trades WHERE status = 'closed')::bigint,
  (SELECT COUNT(*) FROM reconciliation_snapshots)::bigint;" 2>/dev/null || echo '0|0|0')

OPEN_POSITIONS=$(echo "$DB_COUNTS" | awk -F'|' '{print $1}')
CLOSED_TRADES=$(echo "$DB_COUNTS" | awk -F'|' '{print $2}')
RECON_SNAPSHOTS=$(echo "$DB_COUNTS" | awk -F'|' '{print $3}')

STATUS="passed"
if [[ $ISSUES -gt 0 ]]; then
  STATUS="failed"
fi

cat > "$JSON_FILE" <<JSON
{
  "schema_version": "v1",
  "created_at_utc": "$CREATED_AT",
  "status": "$STATUS",
  "checks": {
    "health_check": $HEALTH_OK,
    "api_performance_consistency": $PERF_OK,
    "redis_subscribers_ok": $REDIS_OK
  },
  "redis": {
    "market_data_subscribers": $MD_SUB,
    "decision_subscribers": $DEC_SUB
  },
  "db": {
    "open_positions": $OPEN_POSITIONS,
    "closed_trades": $CLOSED_TRADES,
    "reconciliation_snapshots": $RECON_SNAPSHOTS
  },
  "issues": $ISSUES
}
JSON

cat > "$MD_FILE" <<MD
# Phase 4 Paper Regression - ${DATE_UTC}

## Summary

- status: ${STATUS}
- issues: ${ISSUES}
- health_check: ${HEALTH_OK}
- api_performance_consistency: ${PERF_OK}
- redis_subscribers_ok: ${REDIS_OK}
- redis_market_data_subscribers: ${MD_SUB}
- redis_decision_subscribers: ${DEC_SUB}
- db_open_positions: ${OPEN_POSITIONS}
- db_closed_trades: ${CLOSED_TRADES}
- db_reconciliation_snapshots: ${RECON_SNAPSHOTS}

## Artifact

- ${JSON_FILE#$ROOT_DIR/}
MD

echo "Evidence JSON: ${JSON_FILE#$ROOT_DIR/}"
echo "Evidence MD:   ${MD_FILE#$ROOT_DIR/}"

if [[ $ISSUES -eq 0 ]]; then
  echo -e "${GREEN}SUCCESS: Phase 4 paper regression passed${NC}"
  exit 0
fi

echo -e "${YELLOW}WARN: Phase 4 paper regression found ${ISSUES} issue(s)${NC}"
exit 1