#!/bin/bash
# scripts/security-check.sh
# ViperTrade - Security Verification Script

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

OK='[OK]'
WARN='[WARN]'
ERR='[ERR]'
INFO='[INFO]'

echo -e "${GREEN}${INFO} ViperTrade - Security Check${NC}"
echo "================================================"
echo ""

cd "$(dirname "$0")/.."

ISSUES=0

# 1. Check .env permissions
if [[ -f compose/.env ]]; then
    PERMS=$(stat -c %a compose/.env 2>/dev/null || stat -f %A compose/.env)
    if [[ "$PERMS" == "600" ]]; then
        echo -e "${GREEN}${OK} compose/.env has permission 600${NC}"
    else
        echo -e "${RED}${ERR} compose/.env has permission $PERMS (expected 600)${NC}"
        ISSUES=$((ISSUES + 1))
    fi
else
    echo -e "${YELLOW}${WARN} compose/.env not found${NC}"
fi

# 2. Check if .env is in .gitignore
if grep -Eq '(^|/)\.env(\.|$)' .gitignore 2>/dev/null; then
    echo -e "${GREEN}${OK} .env pattern found in .gitignore${NC}"
else
    echo -e "${RED}${ERR} .env pattern missing in .gitignore${NC}"
    ISSUES=$((ISSUES + 1))
fi

# 3. Check if .env is tracked by Git
if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    if git ls-files --error-unmatch compose/.env > /dev/null 2>&1; then
        echo -e "${RED}${ERR} CRITICAL: compose/.env is tracked by Git${NC}"
        ISSUES=$((ISSUES + 1))
    else
        echo -e "${GREEN}${OK} compose/.env is not tracked by Git${NC}"
    fi
else
    echo -e "${YELLOW}${WARN} Git repository not detected (tracking check skipped)${NC}"
fi

# 4. Check secrets directory
if [[ -d secrets ]]; then
    PERMS=$(stat -c %a secrets 2>/dev/null || stat -f %A secrets)
    if [[ "$PERMS" == "700" ]]; then
        echo -e "${GREEN}${OK} secrets/ has permission 700${NC}"
    else
        echo -e "${YELLOW}${WARN} secrets/ has permission $PERMS (recommended 700)${NC}"
    fi
fi

# 5. Check for hardcoded secrets in code
echo -e "${YELLOW}${INFO} Checking for hardcoded secrets...${NC}"
if grep -RIn --exclude-dir=.git --exclude-dir=target \
    --include='*.rs' --include='*.py' --include='*.toml' --include='*.yaml' --include='*.yml' \
    'sk_live_' . >/dev/null 2>&1; then
    echo -e "${RED}${ERR} Possible hardcoded secret found${NC}"
    ISSUES=$((ISSUES + 1))
else
    echo -e "${GREEN}${OK} No hardcoded secret detected${NC}"
fi

# 6. Check for API keys in code
if grep -RInE --exclude-dir=.git --exclude-dir=target --exclude='*.env*' \
    --include='*.rs' --include='*.py' --include='*.toml' --include='*.yaml' --include='*.yml' \
    '(BYBIT_API_KEY|BYBIT_API_SECRET)=[A-Za-z0-9]+' . >/dev/null 2>&1; then
    echo -e "${RED}${ERR} Possible hardcoded API key found${NC}"
    ISSUES=$((ISSUES + 1))
else
    echo -e "${GREEN}${OK} No hardcoded API key detected${NC}"
fi

# 7. Check scripts are executable
if [[ -x scripts/init-secrets.sh ]] && [[ -x scripts/security-check.sh ]]; then
    echo -e "${GREEN}${OK} Key scripts are executable${NC}"
else
    echo -e "${YELLOW}${WARN} Some scripts are not executable${NC}"
fi

# 8. Check for sensitive data in logs
if [[ -d logs ]]; then
    if grep -rE '(api_key|api_secret|password|secret)' logs/ 2>/dev/null | head -5 | grep -q .; then
        echo -e "${RED}${ERR} Possible sensitive data found in logs${NC}"
        ISSUES=$((ISSUES + 1))
    else
        echo -e "${GREEN}${OK} No sensitive data detected in logs${NC}"
    fi
fi

# Summary
echo ""
echo "================================================"
if [[ $ISSUES -eq 0 ]]; then
    echo -e "${GREEN}${OK} All security checks passed${NC}"
else
    echo -e "${RED}${ERR} $ISSUES issue(s) found${NC}"
fi
echo ""

exit $ISSUES