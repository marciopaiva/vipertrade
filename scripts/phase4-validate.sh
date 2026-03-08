#!/usr/bin/env bash
set -euo pipefail

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
DATE_UTC="$(date -u +%Y-%m-%d)"
OUT_FILE="$ROOT_DIR/docs/operations/PHASE4_BASELINE_${DATE_UTC}.md"

cd "$ROOT_DIR"

echo -e "${GREEN}ViperTrade - Phase 4 Baseline Validation${NC}"
echo "================================================"

ISSUES=0

if ./scripts/health-check.sh >/tmp/viper_phase4_health.log 2>&1; then
  echo -e "${GREEN}OK: health-check passed${NC}"
else
  echo -e "${RED}ERROR: health-check failed${NC}"
  tail -n 80 /tmp/viper_phase4_health.log || true
  ISSUES=$((ISSUES + 1))
fi

BACKTEST_HTTP="$(curl -s -o /tmp/viper_phase4_backtest.out -w "%{http_code}" http://localhost:8085/health || true)"
if [[ "$BACKTEST_HTTP" == "200" ]]; then
  echo -e "${GREEN}OK: backtest service reachable (/health)${NC}"
else
  echo -e "${RED}ERROR: backtest service not reachable (HTTP $BACKTEST_HTTP)${NC}"
  ISSUES=$((ISSUES + 1))
fi

if cargo check -p viper-backtest >/tmp/viper_phase4_backtest_check.log 2>&1; then
  echo -e "${GREEN}OK: cargo check -p viper-backtest${NC}"
else
  echo -e "${RED}ERROR: cargo check -p viper-backtest failed${NC}"
  tail -n 80 /tmp/viper_phase4_backtest_check.log || true
  ISSUES=$((ISSUES + 1))
fi

mkdir -p docs/operations
cat > "$OUT_FILE" <<EOF
# Phase 4 Baseline Validation - ${DATE_UTC}

## Summary

- Health check: executed
- Backtest endpoint /health HTTP: ${BACKTEST_HTTP}
- Backtest crate check: executed
- Issues found: ${ISSUES}

## Notes

- This is a baseline gate for Phase 4 initialization.
- Follow-up: add deterministic backtest run command and artifact contract.
EOF

if [[ $ISSUES -eq 0 ]]; then
  echo -e "${GREEN}SUCCESS: Phase 4 baseline validation passed${NC}"
  echo "Evidence: $OUT_FILE"
  exit 0
fi

echo -e "${YELLOW}WARN: Phase 4 baseline validation found ${ISSUES} issue(s)${NC}"
echo "Evidence: $OUT_FILE"
exit 1