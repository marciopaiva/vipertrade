#!/bin/bash
# scripts/validate-pipeline.sh
# ViperTrade - Tupa Pipeline Validation

set -euo pipefail

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

PIPELINE_FILE='config/strategies/viper_smart_copy.tp'
CHECK_LOG='/tmp/tupa_check.log'
AST_OUT='/tmp/viper_smart_copy.ast.json'

echo -e "${GREEN}ViperTrade - Pipeline Validation${NC}"
echo "================================================"

cd "$(dirname "$0")/.."

if [[ ! -f "$PIPELINE_FILE" ]]; then
    echo -e "${RED}ERROR: pipeline file not found: $PIPELINE_FILE${NC}"
    exit 1
fi
echo -e "${GREEN}OK: pipeline file exists${NC}"

TUPA_BIN="${TUPA_BIN:-tupa}"
if ! command -v "$TUPA_BIN" >/dev/null 2>&1; then
    echo -e "${RED}ERROR: '$TUPA_BIN' not found in PATH${NC}"
    echo -e "${YELLOW}Hint: install tupa CLI or set TUPA_BIN=/path/to/tupa${NC}"
    exit 1
fi
echo -e "${BLUE}Using Tupa CLI: $(command -v "$TUPA_BIN")${NC}"

echo "Checking syntax and types..."
if "$TUPA_BIN" check "$PIPELINE_FILE" 2>&1 | tee "$CHECK_LOG"; then
    echo -e "${GREEN}OK: syntax/type check passed${NC}"
else
    echo -e "${RED}ERROR: syntax/type check failed${NC}"
    cat "$CHECK_LOG"
    exit 1
fi

echo "Parsing AST as JSON..."
if "$TUPA_BIN" parse --format json "$PIPELINE_FILE" > "$AST_OUT"; then
    echo -e "${GREEN}OK: AST generated${NC}"
else
    echo -e "${RED}ERROR: AST generation failed${NC}"
    exit 1
fi

if [[ ! -s "$AST_OUT" ]]; then
    echo -e "${RED}ERROR: generated AST is empty${NC}"
    exit 1
fi

echo -e "${GREEN}OK: AST output exists ($(du -h "$AST_OUT" | awk '{print $1}'))${NC}"

if command -v jq >/dev/null 2>&1; then
    if jq empty "$AST_OUT" >/dev/null 2>&1; then
        echo -e "${GREEN}OK: AST JSON is valid${NC}"
    else
        echo -e "${RED}ERROR: AST JSON is invalid${NC}"
        exit 1
    fi

    if jq -e '.items and (.items | length > 0)' "$AST_OUT" >/dev/null 2>&1; then
        echo -e "${GREEN}OK: AST has at least one item${NC}"
    else
        echo -e "${RED}ERROR: AST has no items${NC}"
        exit 1
    fi
else
    echo -e "${YELLOW}WARN: jq not installed; skipped JSON field validation${NC}"
fi

rm -f "$CHECK_LOG"

echo ""
echo -e "${GREEN}SUCCESS: pipeline validation complete${NC}"
