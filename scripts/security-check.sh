#!/bin/bash
set -euo pipefail

GREEN="\033[0;32m"
RED="\033[0;31m"
YELLOW="\033[1;33m"
NC="\033[0m"

echo -e "${GREEN}ViperTrade - Security Check${NC}"
echo "================================================"

ISSUES=0

# 1) .env exists and has restrictive permissions
if [[ -f compose/.env ]]; then
  PERMS=$(stat -c "%a" compose/.env)
  if [[ "$PERMS" == "600" ]]; then
    echo -e "${GREEN}OK: compose/.env permission is 600${NC}"
  else
    echo -e "${YELLOW}WARN: compose/.env permission is ${PERMS} (recommended 600)${NC}"
  fi
else
  echo -e "${RED}ERROR: compose/.env not found${NC}"
  ISSUES=$((ISSUES + 1))
fi

# 2) ensure .env is ignored by git
if grep -q "^\*\*/\.env" .gitignore; then
  echo -e "${GREEN}OK: .env ignore rule present${NC}"
else
  echo -e "${RED}ERROR: .env ignore rule missing in .gitignore${NC}"
  ISSUES=$((ISSUES + 1))
fi

# 3) ensure .env is not tracked
if git ls-files --error-unmatch compose/.env >/dev/null 2>&1; then
  echo -e "${RED}ERROR: compose/.env is tracked by git${NC}"
  ISSUES=$((ISSUES + 1))
else
  echo -e "${GREEN}OK: compose/.env is not tracked${NC}"
fi

# 4) quick hardcoded-secret scan in relevant source directories
echo "Scanning for possible hardcoded secrets..."
if grep -RInE "(api[_-]?key|api[_-]?secret|password|token)\s*[:=]\s*[\"\x27][^\"\x27]{8,}[\"\x27]" services config 2>/dev/null | grep -v ".env"; then
  echo -e "${YELLOW}WARN: potential secret-like literals found (review output above)${NC}"
else
  echo -e "${GREEN}OK: no obvious hardcoded secrets detected${NC}"
fi

# 5) secrets dir permission recommendation
if [[ -d secrets ]]; then
  SPERMS=$(stat -c "%a" secrets)
  if [[ "$SPERMS" == "700" ]]; then
    echo -e "${GREEN}OK: secrets/ permission is 700${NC}"
  else
    echo -e "${YELLOW}WARN: secrets/ permission is ${SPERMS} (recommended 700)${NC}"
  fi
fi

echo ""
if [[ $ISSUES -eq 0 ]]; then
  echo -e "${GREEN}SUCCESS: critical security checks passed${NC}"
else
  echo -e "${RED}FAILED: ${ISSUES} critical issue(s) found${NC}"
fi

exit $ISSUES
