#!/bin/bash
# scripts/init-secrets.sh
# ViperTrade v0.8.0-rc - Secrets Initialization with Tupa Support

set -euo pipefail

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

show_help() {
    echo "ViperTrade - Secrets Initialization"
    echo "=================================="
    echo ""
    echo "Usage:"
    echo "  ./scripts/init-secrets.sh"
    echo ""
    echo "Description:"
    echo "  Creates or refreshes compose/.env from the template, secures permissions,"
    echo "  creates required directories, and fills generated secrets when needed."
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" || "${1:-}" == "help" ]]; then
    show_help
    exit 0
fi

echo -e "${GREEN}🐍 ViperTrade v0.8.0-rc - Secrets Initialization${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

cd "$(dirname "$0")/.."

# Load version
if [[ -f VERSION ]]; then
    source VERSION
    echo -e "${BLUE}📦 Version: VIPERTRADE=${VIPERTRADE_VERSION:-unknown}, TUPA=${TUPA_VERSION:-unknown}${NC}"
fi

# 1. Check if .env exists
if [[ -f compose/.env ]]; then
    echo -e "${YELLOW}⚠️  compose/.env already exists${NC}"
    # Non-interactive mode fallback if needed, but here we assume interactive or user intent
    if [[ -t 0 ]]; then
        read -p "Create a backup and overwrite it? (y/N): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            echo -e "${GREEN}✓ Keeping existing .env${NC}"
        else
            cp compose/.env "compose/.env.backup.$(date +%Y%m%d_%H%M%S)"
            echo -e "${GREEN}✓ Backup created${NC}"
            cp compose/.env.example compose/.env
            echo -e "${GREEN}✓ compose/.env recreated from the template${NC}"
        fi
    else
         # In non-interactive mode (like here), we generally want to preserve unless forced. 
         # But the user script implies we should ensure it exists.
         # If it exists, we skip overwriting to be safe, unless it's empty.
         echo -e "${GREEN}✓ Keeping existing .env${NC}"
    fi
else
    # 2. Create .env from example
    cp compose/.env.example compose/.env
    echo -e "${GREEN}✓ compose/.env created from the template${NC}"
fi

# 3. Set secure permissions
chmod 600 compose/.env
echo -e "${GREEN}✓ compose/.env permissions set to 600${NC}"

# 4. Create secrets directory
mkdir -p secrets
chmod 700 secrets
echo -e "${GREEN}✓ secrets/ directory created with 700 permissions${NC}"

# 5. Create logs and audit directories
mkdir -p logs/audit
chmod 755 logs
chmod 700 logs/audit
echo -e "${GREEN}✓ logs/ and logs/audit/ directories created${NC}"

# 6. Verify Git protection
if git rev-parse --is-inside-work-tree > /dev/null 2>&1; then
    echo -e "${BLUE}🔍 Checking Git protection...${NC}"
    if ! grep -q "^\*\*/\.env" .gitignore; then
        echo -e "${RED}✗ WARNING: .env may not be protected in .gitignore${NC}"
    else
        echo -e "${GREEN}✓ .env is protected in .gitignore${NC}"
    fi
    
    if git ls-files --error-unmatch compose/.env > /dev/null 2>&1; then
        echo -e "${RED}🚨 CRITICAL: compose/.env is tracked by Git${NC}"
        echo "   Run:"
        echo "   git rm --cached compose/.env"
        echo "   git commit -m 'Remove .env from tracking'"
        exit 1
    else
        echo -e "${GREEN}✓ compose/.env is not tracked by Git${NC}"
    fi
fi

# 7. Generate strong passwords
for var in DB_PASSWORD NEXTAUTH_SECRET; do
    if grep -q "${var}=generate_" compose/.env; then
        echo -e "${YELLOW}⚠️  Generating ${var}...${NC}"
        VALUE=$(openssl rand -base64 32 | tr -dc 'a-zA-Z0-9' | head -c 32)
        if [[ "$OSTYPE" == "darwin"* ]]; then
            sed -i '' "s|${var}=generate_.*|${var}=${VALUE}|" compose/.env
        else
            sed -i "s|${var}=generate_.*|${var}=${VALUE}|" compose/.env
        fi
        echo -e "${GREEN}✓ ${var} generated${NC}"
    fi
done

# 7.1 Keep compose variables compatible with DB_* naming used in template
if ! grep -q '^POSTGRES_DB=' compose/.env; then
    echo "POSTGRES_DB=${DB_NAME:-vipertrade}" >> compose/.env
fi
if ! grep -q '^POSTGRES_USER=' compose/.env; then
    echo "POSTGRES_USER=${DB_USER:-viper}" >> compose/.env
fi
if ! grep -q '^POSTGRES_PASSWORD=' compose/.env; then
    DB_PASSWORD_VALUE="$(grep '^DB_PASSWORD=' compose/.env | head -n1 | cut -d'=' -f2-)"
    if [[ -n "$DB_PASSWORD_VALUE" ]]; then
        echo "POSTGRES_PASSWORD=${DB_PASSWORD_VALUE}" >> compose/.env
    fi
fi

# 7.2 Ensure standardized base-image variables exist
if ! grep -q '^RUST_BUILDER_IMAGE=' compose/.env; then
    echo "RUST_BUILDER_IMAGE=vipertrade-base-rust-builder:1.83" >> compose/.env
fi
if ! grep -q '^RUST_RUNTIME_IMAGE=' compose/.env; then
    echo "RUST_RUNTIME_IMAGE=vipertrade-base-rust-runtime:bookworm" >> compose/.env
fi
if ! grep -q '^STRATEGY_BUILDER_IMAGE=' compose/.env; then
    echo "STRATEGY_BUILDER_IMAGE=vipertrade-base-strategy-builder:1.83" >> compose/.env
fi
if ! grep -q '^STRATEGY_RUNTIME_IMAGE=' compose/.env; then
    echo "STRATEGY_RUNTIME_IMAGE=vipertrade-base-strategy-runtime:3.12-bookworm" >> compose/.env
fi
if ! grep -q '^WEB_BASE_IMAGE=' compose/.env; then
    echo "WEB_BASE_IMAGE=vipertrade-base-web-node:20-bookworm" >> compose/.env
fi

# Load envs for display
set +u # Allow unbound variables for display
source compose/.env

# 8. Display Tupa config
echo ""
echo -e "${BLUE}🤖 Tupa configuration:${NC}"
echo "   Version: ${TUPA_VERSION:-0.8.0-rc}"
echo "   Backend: ${TUPA_BACKEND:-hybrid}"
echo "   Pipeline: ${TUPA_PIPELINE_PATH:-/app/config/strategies/viper_smart_copy.tp}"
echo "   Audit Path: ${TUPA_AUDIT_PATH:-/app/logs/audit}"

# 9. Display trading config
echo ""
echo -e "${BLUE}📊 Trading configuration:${NC}"
echo "   Mode: ${TRADING_MODE:-paper}"
echo "   Profile: ${TRADING_PROFILE:-MEDIUM}"
echo "   Pairs: ${TRADING_PAIRS:-from config/trading/pairs.yaml}"
echo "   Smart Copy: ${SMART_COPY_ENABLED:-true}"
echo "   Trailing Stop: ${TRAILING_STOP_ENABLED:-true}"

# 10. Final instructions
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo -e "${GREEN}✅ Secrets initialized successfully${NC}"
echo ""
echo -e "${YELLOW}📋 NEXT STEPS:${NC}"
echo "   1. Edit ${GREEN}compose/.env${NC} with your Bybit credentials"
echo "   2. Create Discord webhooks in: Discord → Channel → Integrations"
echo "   3. Run: ${GREEN}./scripts/compose.sh up -d --build${NC}"
echo "   4. Monitor: ${GREEN}./scripts/compose.sh logs -f strategy${NC}"
echo ""
echo -e "${YELLOW}🔐 USEFUL COMMANDS:${NC}"
echo "   Security:   ./scripts/security-check.sh"
echo "   Health:     ./scripts/health-check.sh"
echo "   Build Tupa: ./scripts/build-tupa.sh"
echo "   Logs:       ./scripts/compose.sh logs -f"
echo ""

exit 0
