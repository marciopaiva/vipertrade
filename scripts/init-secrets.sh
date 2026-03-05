#!/bin/bash
# scripts/init-secrets.sh
# ViperTrade - Secrets Initialization with Security Checks

set -euo pipefail

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

OK='[OK]'
WARN='[WARN]'
ERR='[ERR]'
INFO='[INFO]'

echo -e "${GREEN}${INFO} ViperTrade - Secrets Initialization${NC}"
echo "================================================"
echo ""

cd "$(dirname "$0")/.."

require_cmd() {
    local cmd="$1"
    local hint="${2:-}"
    if ! command -v "$cmd" >/dev/null 2>&1; then
        echo -e "${RED}${ERR} Missing dependency: $cmd${NC}"
        [[ -n "$hint" ]] && echo -e "${YELLOW}${WARN} $hint${NC}"
        exit 1
    fi
}

compose_cmd() {
    if command -v podman-compose >/dev/null 2>&1; then
        echo "podman-compose"
        return
    fi

    if command -v podman >/dev/null 2>&1 && podman compose version >/dev/null 2>&1; then
        echo "podman compose"
        return
    fi

    echo ""
}

require_cmd chmod
require_cmd cp
require_cmd grep
require_cmd mkdir
require_cmd sed

COMPOSE_CMD="$(compose_cmd)"
if [[ -z "$COMPOSE_CMD" ]]; then
    echo -e "${RED}${ERR} Podman Compose not found${NC}"
    echo -e "${YELLOW}${WARN} Install podman-compose or enable 'podman compose'${NC}"
    exit 1
fi

if [[ ! -f compose/.env.example ]]; then
    echo -e "${RED}${ERR} compose/.env.example not found${NC}"
    exit 1
fi

# 1. Check if .env exists
if [[ -f compose/.env ]]; then
    echo -e "${YELLOW}${WARN} compose/.env already exists${NC}"
    read -p "Create backup and overwrite? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        cp compose/.env "compose/.env.backup.$(date +%Y%m%d_%H%M%S)"
        cp compose/.env.example compose/.env
        echo -e "${GREEN}${OK} Backup created and .env replaced from template${NC}"
    else
        echo -e "${GREEN}${OK} Keeping existing .env${NC}"
    fi
fi

# 2. Create .env from example
if [[ ! -f compose/.env ]]; then
    cp compose/.env.example compose/.env
    echo -e "${GREEN}${OK} compose/.env created from template${NC}"
fi

# 3. Set secure permissions
chmod 600 compose/.env
echo -e "${GREEN}${OK} compose/.env permissions set to 600${NC}"

# 4. Create secrets directory
mkdir -p secrets
chmod 700 secrets
echo -e "${GREEN}${OK} secrets/ created with permission 700${NC}"

# 5. Create logs directory
mkdir -p logs
chmod 755 logs
echo -e "${GREEN}${OK} logs/ created${NC}"

# 6. Verify Git protection
if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    echo -e "${BLUE}${INFO} Checking Git protection...${NC}"
    if ! grep -Eq '(^|/)\.env(\.|$)' .gitignore 2>/dev/null; then
        echo -e "${RED}${ERR} WARNING: .env pattern may be missing in .gitignore${NC}"
    else
        echo -e "${GREEN}${OK} .env pattern found in .gitignore${NC}"
    fi

    if git ls-files --error-unmatch compose/.env > /dev/null 2>&1; then
        echo -e "${RED}${ERR} CRITICAL: compose/.env is tracked by Git${NC}"
        echo "   Run:"
        echo "   git rm --cached compose/.env"
        echo "   git commit -m 'Remove .env from tracking'"
        exit 1
    else
        echo -e "${GREEN}${OK} compose/.env is not tracked by Git${NC}"
    fi
else
    echo -e "${YELLOW}${WARN} Git repository not detected (tracking check skipped)${NC}"
fi

# 7. Generate strong password if not set
if grep -q "POSTGRES_PASSWORD=generate_strong_password_here" compose/.env; then
    require_cmd openssl "Install openssl to auto-generate secrets"
    echo -e "${YELLOW}${INFO} Generating strong PostgreSQL password...${NC}"
    STRONG_PASS=$(openssl rand -base64 32 | tr -dc 'a-zA-Z0-9' | head -c 32)
    if [[ "$OSTYPE" == "darwin"* ]]; then
        sed -i '' "s/POSTGRES_PASSWORD=generate_strong_password_here/POSTGRES_PASSWORD=${STRONG_PASS}/" compose/.env
    else
        sed -i "s/POSTGRES_PASSWORD=generate_strong_password_here/POSTGRES_PASSWORD=${STRONG_PASS}/" compose/.env
    fi
    echo -e "${GREEN}${OK} Strong password generated${NC}"
fi

# 8. Generate NEXTAUTH_SECRET if not set
if grep -q "NEXTAUTH_SECRET=generate_nextauth_secret_here" compose/.env; then
    require_cmd openssl "Install openssl to auto-generate secrets"
    echo -e "${YELLOW}${INFO} Generating NEXTAUTH_SECRET...${NC}"
    AUTH_SECRET=$(openssl rand -base64 32)
    if [[ "$OSTYPE" == "darwin"* ]]; then
        sed -i '' "s/NEXTAUTH_SECRET=generate_nextauth_secret_here/NEXTAUTH_SECRET=${AUTH_SECRET}/" compose/.env
    else
        sed -i "s/NEXTAUTH_SECRET=generate_nextauth_secret_here/NEXTAUTH_SECRET=${AUTH_SECRET}/" compose/.env
    fi
    echo -e "${GREEN}${OK} NEXTAUTH_SECRET generated${NC}"
fi

# 9. Display trading config
set -a
# shellcheck source=/dev/null
source compose/.env
set +a

echo ""
echo -e "${BLUE}${INFO} Trading configuration:${NC}"
echo "   Mode: ${TRADING_MODE:-paper}"
echo "   Profile: ${TRADING_PROFILE:-MEDIUM}"
echo "   Pairs: ${TRADING_PAIRS:-DOGEUSDT,XRPUSDT,TRXUSDT,XLMUSDT}"
echo "   Smart Copy: ${SMART_COPY_ENABLED:-true}"
echo "   Trailing Stop: ${TRAILING_STOP_ENABLED:-true}"

# 10. Final instructions
echo ""
echo "================================================"
echo -e "${GREEN}${OK} Secrets initialized securely${NC}"
echo ""
echo -e "${YELLOW}${INFO} NEXT STEPS:${NC}"
echo -e "   1. Edit ${GREEN}compose/.env${NC} with your Bybit credentials"
echo "   2. Create Discord webhooks in your channel integrations"
echo -e "   3. Run: ${GREEN}cd compose && ${COMPOSE_CMD} up --build${NC}"
echo -e "   4. Monitor: ${GREEN}${COMPOSE_CMD} logs -f${NC}"
echo ""
echo -e "${YELLOW}${INFO} USEFUL COMMANDS:${NC}"
echo "   Security check: ./scripts/security-check.sh"
echo "   View logs:      ${COMPOSE_CMD} logs -f"
echo "   Stop all:       ${COMPOSE_CMD} down"
echo "   Health check:   ${COMPOSE_CMD} ps"
echo ""

exit 0