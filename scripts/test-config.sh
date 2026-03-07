#!/bin/bash
# scripts/test-config.sh
# ViperTrade - Configuration Validation

set -euo pipefail

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${GREEN}⚙️  ViperTrade - Configuration Validation${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

cd "$(dirname "$0")/.."

ISSUES=0

# 1. Check pairs.yaml
echo "🔍 Checking config/trading/pairs.yaml..."
if [[ -f config/trading/pairs.yaml ]]; then
    # Validate YAML syntax (if yq installed)
    if command -v yq &> /dev/null; then
        if yq eval '.' config/trading/pairs.yaml > /dev/null 2>&1; then
            echo -e "${GREEN}✓ pairs.yaml syntax valid${NC}"
        else
            echo -e "${RED}✗ pairs.yaml syntax errors${NC}"
            ISSUES=$((ISSUES + 1))
        fi
    else
        echo -e "${YELLOW}⚠ yq not installed - skipping YAML validation${NC}"
    fi
    
    # Check required pairs
    for pair in DOGEUSDT XRPUSDT TRXUSDT XLMUSDT; do
        if grep -q "$pair" config/trading/pairs.yaml; then
            echo -e "${GREEN}✓ $pair configured${NC}"
        else
            echo -e "${RED}✗ $pair missing${NC}"
            ISSUES=$((ISSUES + 1))
        fi
    done
    
    # Check Smart Copy settings
    if grep -q "smart_copy:" config/trading/pairs.yaml; then
        echo -e "${GREEN}✓ Smart Copy settings present${NC}"
    else
        echo -e "${YELLOW}⚠ Smart Copy settings missing${NC}"
    fi
    
    # Check Trailing Stop settings
    if grep -q "trailing_stop:" config/trading/pairs.yaml; then
        echo -e "${GREEN}✓ Trailing Stop settings present${NC}"
    else
        echo -e "${YELLOW}⚠ Trailing Stop settings missing${NC}"
    fi
else
    echo -e "${RED}✗ config/trading/pairs.yaml not found${NC}"
    ISSUES=$((ISSUES + 1))
fi

# 2. Check pipeline
echo ""
echo "🔍 Checking config/strategies/viper_smart_copy.tp..."
if [[ -f config/strategies/viper_smart_copy.tp ]]; then
    echo -e "${GREEN}✓ Pipeline file exists${NC}"
    
    # Check for required Tupã v0.8.0-rc features
    if grep -q "@constraints" config/strategies/viper_smart_copy.tp; then
        echo -e "${GREEN}✓ @constraints attribute found${NC}"
    else
        echo -e "${RED}✗ @constraints attribute missing${NC}"
        ISSUES=$((ISSUES + 1))
    fi
    
    if grep -q "@validate" config/strategies/viper_smart_copy.tp; then
        echo -e "${GREEN}✓ @validate attribute found${NC}"
    else
        echo -e "${RED}✗ @validate attribute missing${NC}"
        ISSUES=$((ISSUES + 1))
    fi
    
    if grep -q "log_decision" config/strategies/viper_smart_copy.tp; then
        echo -e "${GREEN}✓ Audit logging enabled${NC}"
    else
        echo -e "${YELLOW}⚠ Audit logging not found${NC}"
    fi
    
    if grep -q "ViperSmartCopy" config/strategies/viper_smart_copy.tp; then
        echo -e "${GREEN}✓ Pipeline name correct${NC}"
    else
        echo -e "${RED}✗ Pipeline name incorrect${NC}"
        ISSUES=$((ISSUES + 1))
    fi
else
    echo -e "${RED}✗ config/strategies/viper_smart_copy.tp not found${NC}"
    ISSUES=$((ISSUES + 1))
fi

# 3. Check profiles
echo ""
echo "🔍 Checking config/system/profiles.yaml..."
if [[ -f config/system/profiles.yaml ]]; then
    echo -e "${GREEN}✓ profiles.yaml exists${NC}"
    
    for profile in CONSERVATIVE MEDIUM AGGRESSIVE; do
        if grep -q "$profile:" config/system/profiles.yaml; then
            echo -e "${GREEN}✓ $profile profile configured${NC}"
        else
            echo -e "${RED}✗ $profile profile missing${NC}"
            ISSUES=$((ISSUES + 1))
        fi
    done
else
    echo -e "${YELLOW}⚠ config/system/profiles.yaml not found (optional)${NC}"
fi

# Summary
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
if [[ $ISSUES -eq 0 ]]; then
    echo -e "${GREEN}✅ All configuration checks passed!${NC}"
else
    echo -e "${RED}⚠️  $ISSUES issue(s) found - review configuration${NC}"
fi
echo ""

exit $ISSUES