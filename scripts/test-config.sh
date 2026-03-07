#!/bin/bash
set -euo pipefail

GREEN="\033[0;32m"
RED="\033[0;31m"
YELLOW="\033[1;33m"
NC="\033[0m"

echo -e "${GREEN}ViperTrade - Configuration Validation${NC}"
echo "================================================"

cd "$(dirname "$0")/.."
ISSUES=0

check_file() {
  local file="$1"
  if [[ -f "$file" ]]; then
    echo -e "${GREEN}OK: $file exists${NC}"
  else
    echo -e "${RED}ERROR: $file not found${NC}"
    ISSUES=$((ISSUES + 1))
  fi
}

check_file "config/trading/pairs.yaml"
check_file "config/system/profiles.yaml"
check_file "config/strategies/viper_smart_copy.tp"

if [[ -f config/trading/pairs.yaml ]] && command -v yq >/dev/null 2>&1; then
  if yq eval "." config/trading/pairs.yaml >/dev/null 2>&1; then
    echo -e "${GREEN}OK: pairs.yaml syntax valid${NC}"
  else
    echo -e "${RED}ERROR: pairs.yaml syntax invalid${NC}"
    ISSUES=$((ISSUES + 1))
  fi
else
  echo -e "${YELLOW}WARN: yq not installed; skipping YAML syntax check${NC}"
fi

if [[ -f config/system/profiles.yaml ]] && command -v yq >/dev/null 2>&1; then
  if yq eval "." config/system/profiles.yaml >/dev/null 2>&1; then
    echo -e "${GREEN}OK: profiles.yaml syntax valid${NC}"
  else
    echo -e "${RED}ERROR: profiles.yaml syntax invalid${NC}"
    ISSUES=$((ISSUES + 1))
  fi
fi

if [[ -f config/system/profiles.yaml ]]; then
  for profile in CONSERVATIVE MEDIUM AGGRESSIVE; do
    if grep -q "^${profile}:" config/system/profiles.yaml; then
      echo -e "${GREEN}OK: profile ${profile} configured${NC}"
    else
      echo -e "${YELLOW}WARN: profile ${profile} missing${NC}"
    fi
  done
fi

if [[ -x scripts/validate-pipeline.sh ]]; then
  if ./scripts/validate-pipeline.sh >/tmp/viper_validate_pipeline.log 2>&1; then
    echo -e "${GREEN}OK: pipeline validation passed${NC}"
  else
    echo -e "${RED}ERROR: pipeline validation failed${NC}"
    tail -n 40 /tmp/viper_validate_pipeline.log || true
    ISSUES=$((ISSUES + 1))
  fi
else
  echo -e "${RED}ERROR: scripts/validate-pipeline.sh missing or not executable${NC}"
  ISSUES=$((ISSUES + 1))
fi

echo ""
if [[ $ISSUES -eq 0 ]]; then
  echo -e "${GREEN}SUCCESS: configuration checks passed${NC}"
else
  echo -e "${RED}FAILED: ${ISSUES} issue(s) found${NC}"
fi

exit $ISSUES
