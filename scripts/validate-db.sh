#!/bin/bash
# scripts/validate-db.sh
# ViperTrade - Database Schema Validation

set -euo pipefail

GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${GREEN}🗄️  ViperTrade - Database Validation${NC}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

cd "$(dirname "$0")/.."

# Check execution method (psql or podman)
EXEC_CMD=""
if command -v psql &> /dev/null; then
    EXEC_CMD="psql"
elif command -v podman &> /dev/null; then
    EXEC_CMD="podman exec -i vipertrade-postgres psql"
elif command -v docker &> /dev/null; then
    EXEC_CMD="docker exec -i vipertrade-postgres psql"
else
    echo -e "${RED}✗ No psql, podman, or docker found${NC}"
    exit 1
fi

# Load env if exists
if [[ -f compose/.env ]]; then
    source compose/.env
fi

DB_HOST="${DB_HOST:-localhost}"
DB_PORT="${DB_PORT:-5432}"
DB_NAME="${DB_NAME:-vipertrade}"
DB_USER="${DB_USER:-viper}"
DB_PASSWORD="${DB_PASSWORD:-}"

# Adjust host for container execution
if [[ "$EXEC_CMD" == *"exec"* ]]; then
    # When running inside container, host is localhost (or socket) and no password needed if trusting local
    # But usually we still need user/db.
    # We'll construct the command slightly differently.
    CMD_PREFIX="$EXEC_CMD -U $DB_USER -d $DB_NAME"
else
    CMD_PREFIX="PGPASSWORD=$DB_PASSWORD psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME"
fi

echo "🔍 Connecting to database..."

# Test connection
if ! $CMD_PREFIX -c "SELECT 1" &> /dev/null; then
    echo -e "${RED}✗ Cannot connect to database${NC}"
    echo "   Check: host, port, credentials, network, container status"
    exit 1
fi
echo -e "${GREEN}✓ Database connection OK${NC}"

# Check required tables
REQUIRED_TABLES=("trades" "position_snapshots" "system_events" "daily_metrics" "tupa_audit_logs" "profile_history" "circuit_breaker_events" "schema_migrations")

for table in "${REQUIRED_TABLES[@]}"; do
    if $CMD_PREFIX -c "SELECT 1 FROM $table LIMIT 1" &> /dev/null; then
        echo -e "${GREEN}✓ Table $table exists${NC}"
    else
        echo -e "${RED}✗ Table $table missing${NC}"
    fi
done

# Check Tupã audit logging table specifically
if $CMD_PREFIX -c "SELECT column_name FROM information_schema.columns WHERE table_name = 'tupa_audit_logs' AND column_name = 'decision_hash'" | grep -q "decision_hash"; then
    echo -e "${GREEN}✓ Tupã audit logging schema OK${NC}"
else
    echo -e "${YELLOW}⚠ Tupã audit logging columns may be missing${NC}"
fi

# Check indexes
INDEX_COUNT=$($CMD_PREFIX -t -c "SELECT COUNT(*) FROM pg_indexes WHERE schemaname = 'public';" | tr -d ' ')
if [[ $INDEX_COUNT -ge 20 ]]; then
    echo -e "${GREEN}✓ Indexes present ($INDEX_COUNT found)${NC}"
else
    echo -e "${YELLOW}⚠ Low index count ($INDEX_COUNT) - check schema${NC}"
fi

# Check migrations
MIGRATION_COUNT=$($CMD_PREFIX -t -c "SELECT COUNT(*) FROM schema_migrations;" | tr -d ' ')
echo -e "${BLUE}ℹ Schema migrations applied: $MIGRATION_COUNT${NC}"

echo ""
echo -e "${GREEN}✅ Database validation complete!${NC}"
exit 0
