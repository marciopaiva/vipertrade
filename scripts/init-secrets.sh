#!/bin/bash
# scripts/init-secrets.sh
# ViperTrade v0.8.0-rc - Secrets Initialization with Tupã Support

set -euo pipefail

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

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
    echo -e "${YELLOW}⚠️  compose/.env já existe${NC}"
    # Non-interactive mode fallback if needed, but here we assume interactive or user intent
    if [[ -t 0 ]]; then
        read -p "Deseja criar backup e sobrescrever? (y/N): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            echo -e "${GREEN}✓ Mantendo .env existente${NC}"
        else
            cp compose/.env "compose/.env.backup.$(date +%Y%m%d_%H%M%S)"
            echo -e "${GREEN}✓ Backup criado${NC}"
            cp compose/.env.example compose/.env
            echo -e "${GREEN}✓ compose/.env recriado a partir do template${NC}"
        fi
    else
         # In non-interactive mode (like here), we generally want to preserve unless forced. 
         # But the user script implies we should ensure it exists.
         # If it exists, we skip overwriting to be safe, unless it's empty.
         echo -e "${GREEN}✓ Mantendo .env existente${NC}"
    fi
else
    # 2. Create .env from example
    cp compose/.env.example compose/.env
    echo -e "${GREEN}✓ compose/.env criado a partir do template${NC}"
fi

# 3. Set secure permissions
chmod 600 compose/.env
echo -e "${GREEN}✓ Permissões de compose/.env definidas para 600${NC}"

# 4. Create secrets directory
mkdir -p secrets
chmod 700 secrets
echo -e "${GREEN}✓ Diretório secrets/ criado com permissão 700${NC}"

# 5. Create logs and audit directories
mkdir -p logs/audit
chmod 755 logs
chmod 700 logs/audit
echo -e "${GREEN}✓ Diretórios logs/ e logs/audit/ criados${NC}"

# 6. Verify Git protection
if git rev-parse --is-inside-work-tree > /dev/null 2>&1; then
    echo -e "${BLUE}🔍 Verificando proteção Git...${NC}"
    if ! grep -q "^\*\*/\.env" .gitignore; then
        echo -e "${RED}✗ ATENÇÃO: .env pode não estar no .gitignore!${NC}"
    else
        echo -e "${GREEN}✓ .env está protegido no .gitignore${NC}"
    fi
    
    if git ls-files --error-unmatch compose/.env > /dev/null 2>&1; then
        echo -e "${RED}🚨 CRÍTICO: compose/.env está no Git!${NC}"
        echo "   Execute:"
        echo "   git rm --cached compose/.env"
        echo "   git commit -m 'Remove .env from tracking'"
        exit 1
    else
        echo -e "${GREEN}✓ compose/.env NÃO está no Git${NC}"
    fi
fi

# 7. Generate strong passwords
for var in DB_PASSWORD NEXTAUTH_SECRET; do
    if grep -q "${var}=generate_" compose/.env; then
        echo -e "${YELLOW}⚠️  Gerando ${var}...${NC}"
        VALUE=$(openssl rand -base64 32 | tr -dc 'a-zA-Z0-9' | head -c 32)
        if [[ "$OSTYPE" == "darwin"* ]]; then
            sed -i '' "s|${var}=generate_.*|${var}=${VALUE}|" compose/.env
        else
            sed -i "s|${var}=generate_.*|${var}=${VALUE}|" compose/.env
        fi
        echo -e "${GREEN}✓ ${var} gerado${NC}"
    fi
done

# Load envs for display
set +u # Allow unbound variables for display
source compose/.env

# 8. Display Tupã config
echo ""
echo -e "${BLUE}🤖 Configuração Tupã:${NC}"
echo "   Version: ${TUPA_VERSION:-0.8.0-rc}"
echo "   Backend: ${TUPA_BACKEND:-hybrid}"
echo "   Pipeline: ${TUPA_PIPELINE_PATH:-/app/config/strategies/viper_smart_copy.tp}"
echo "   Audit Path: ${TUPA_AUDIT_PATH:-/app/logs/audit}"

# 9. Display trading config
echo ""
echo -e "${BLUE}📊 Configuração de Trading:${NC}"
echo "   Mode: ${TRADING_MODE:-paper}"
echo "   Profile: ${TRADING_PROFILE:-MEDIUM}"
echo "   Pairs: ${TRADING_PAIRS:-DOGEUSDT,XRPUSDT,TRXUSDT,XLMUSDT}"
echo "   Smart Copy: ${SMART_COPY_ENABLED:-true}"
echo "   Trailing Stop: ${TRAILING_STOP_ENABLED:-true}"

# 10. Final instructions
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo -e "${GREEN}✅ Secrets inicializados com segurança!${NC}"
echo ""
echo -e "${YELLOW}📋 PRÓXIMOS PASSOS:${NC}"
echo "   1. Edite ${GREEN}compose/.env${NC} com suas credenciais Bybit"
echo "   2. Obtenha Discord webhooks em: Discord → Canal → Integrações"
echo "   3. Execute: ${GREEN}./scripts/compose.sh up -d --build${NC}"
echo "   4. Se bridge falhar no WSL: ${GREEN}./scripts/fix-podman-wsl-network.sh${NC}
   5. Monitore: ${GREEN}./scripts/compose.sh logs -f strategy${NC}"
echo ""
echo -e "${YELLOW}🔐 COMANDOS ÚTEIS:${NC}"
echo "   Security:   ./scripts/security-check.sh"
echo "   Health:     ./scripts/health-check.sh"
echo "   Build Tupã: ./scripts/build-tupa.sh"
echo "   Logs:       ./scripts/compose.sh logs -f"
echo ""

exit 0
