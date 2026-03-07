#!/bin/bash
# scripts/validate-pipeline.sh
# ViperTrade - Tupã Pipeline Validation

set -euo pipefail

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${GREEN}🔍 ViperTrade - Pipeline Validation${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

cd "$(dirname "$0")/.."

# Load version
if [[ -f VERSION ]]; then
    source VERSION
    echo -e "${BLUE}📦 Tupã version: ${TUPA_VERSION:-0.8.0-rc}${NC}"
fi

# Check if pipeline file exists
if [[ ! -f config/strategies/viper_smart_copy.tp ]]; then
    echo -e "${RED}✗ Pipeline file not found: config/strategies/viper_smart_copy.tp${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Pipeline file exists${NC}"

# Validate syntax with tupa-cli
echo "🔍 Validating pipeline syntax..."
if cargo run -p tupa-cli -- check config/strategies/viper_smart_copy.tp 2>&1 | tee /tmp/tupa_check.log; then
    echo -e "${GREEN}✓ Pipeline syntax valid${NC}"
else
    echo -e "${RED}✗ Pipeline syntax errors:${NC}"
    cat /tmp/tupa_check.log
    exit 1
fi

# Generate ExecutionPlan (test compilation)
echo "🔨 Generating ExecutionPlan..."
if cargo run -p tupa-cli -- codegen \
    --backend hybrid \
    --output /tmp/viper_smart_copy.plan.json \
    config/strategies/viper_smart_copy.tp 2>&1 | tee /tmp/tupa_codegen.log; then
    echo -e "${GREEN}✓ ExecutionPlan generated successfully${NC}"
    
    # Verify output file
    if [[ -f /tmp/viper_smart_copy.plan.json ]]; then
        PLAN_SIZE=$(ls -lh /tmp/viper_smart_copy.plan.json | awk '{print $5}')
        echo -e "${GREEN}✓ Plan file size: ${PLAN_SIZE}${NC}"
        
        # Validate JSON structure
        if jq empty /tmp/viper_smart_copy.plan.json 2>/dev/null; then
            echo -e "${GREEN}✓ Plan JSON is valid${NC}"
            
            # Check required fields
            if jq -e '.name' /tmp/viper_smart_copy.plan.json > /dev/null; then
                echo -e "${GREEN}✓ Plan has required fields${NC}"
            fi
        else
            echo -e "${YELLOW}⚠ Could not validate JSON structure (jq not installed)${NC}"
        fi
    fi
else
    echo -e "${RED}✗ ExecutionPlan generation failed:${NC}"
    cat /tmp/tupa_codegen.log
    exit 1
fi

# Check for @constraints attribute (v0.8.0-rc feature)
echo "🔍 Checking for Tupã v0.8.0-rc features..."
if grep -q "@constraints" config/strategies/viper_smart_copy.tp; then
    echo -e "${GREEN}✓ @constraints attribute found (v0.8.0-rc feature)${NC}"
else
    echo -e "${YELLOW}⚠ @constraints attribute not found${NC}"
fi

if grep -q "@validate" config/strategies/viper_smart_copy.tp; then
    echo -e "${GREEN}✓ @validate attribute found (v0.8.0-rc feature)${NC}"
else
    echo -e "${YELLOW}⚠ @validate attribute not found${NC}"
fi

if grep -q "log_decision" config/strategies/viper_smart_copy.tp; then
    echo -e "${GREEN}✓ Audit logging enabled${NC}"
else
    echo -e "${YELLOW}⚠ Audit logging not found${NC}"
fi

# Clean up
rm -f /tmp/tupa_check.log /tmp/tupa_codegen.log

echo ""
echo -e "${GREEN}✅ Pipeline validation complete!${NC}"
echo ""
echo -e "${BLUE}📋 Next steps:${NC}"
echo "   1. Build strategy service: ./scripts/build-tupa.sh"
echo "   2. Run backtest: cargo run -p vipertrade-backtest"
echo "   3. Start system: cd compose && podman-compose up"
echo ""

exit 0