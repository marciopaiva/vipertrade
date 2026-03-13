#!/bin/bash
set -euo pipefail

GREEN="\033[0;32m"
RED="\033[0;31m"
YELLOW="\033[1;33m"
NC="\033[0m"

echo -e "${GREEN}ViperTrade - Health Check${NC}"
echo "================================================"

cd "$(dirname "$0")/.."
. scripts/container-runtime.sh

if [[ ! -x scripts/compose.sh ]]; then
  echo -e "${RED}ERROR: scripts/compose.sh not found${NC}"
  exit 1
fi

echo "Container status:"
./scripts/compose.sh ps || true

echo "Checking Postgres..."
if container_exec vipertrade-postgres pg_isready -U "${POSTGRES_USER:-viper}" >/dev/null 2>&1; then
  echo -e "${GREEN}OK: database ready${NC}"
else
  echo -e "${RED}ERROR: database not ready${NC}"
  exit 1
fi

echo "Checking API endpoint..."
if curl -fsS http://localhost:8080/health >/dev/null; then
  echo -e "${GREEN}OK: API healthy${NC}"
else
  echo -e "${YELLOW}WARN: API health endpoint unreachable${NC}"
fi

echo "Checking Web endpoint..."
if curl -fsS http://localhost:3000 >/dev/null; then
  echo -e "${GREEN}OK: Web reachable${NC}"
else
  echo -e "${YELLOW}WARN: Web endpoint unreachable${NC}"
fi

echo "Checking Web container health..."
WEB_HEALTH=$(container_inspect -f "{{.State.Health.Status}}" vipertrade-web 2>/dev/null || echo "unknown")
if [[ "$WEB_HEALTH" == "healthy" ]]; then
  echo -e "${GREEN}OK: Web container healthy${NC}"
elif [[ "$WEB_HEALTH" == "unknown" ]]; then
  echo -e "${YELLOW}WARN: Web container health status unavailable${NC}"
else
  echo -e "${YELLOW}WARN: Web container status is $WEB_HEALTH (endpoint may still be reachable)${NC}"
fi

echo -e "${GREEN}Health check complete${NC}"
