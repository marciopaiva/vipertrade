#!/bin/bash
# scripts/health-check.sh
# ViperTrade - System Health Check

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

OK='[OK]'
WARN='[WARN]'
ERR='[ERR]'
INFO='[INFO]'

echo -e "${GREEN}${INFO} ViperTrade - Health Check${NC}"
echo "================================================"
echo ""

cd "$(dirname "$0")/.."

ISSUES=0

compose_cmd() {
    if command -v podman-compose >/dev/null 2>&1; then
        echo "podman-compose"
        return
    fi

    if command -v podman >/dev/null 2>&1 && podman compose version >/dev/null 2>&1; then
        echo "podman compose"
        return
    fi

    if command -v docker-compose >/dev/null 2>&1; then
        echo "docker-compose"
        return
    fi

    if command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then
        echo "docker compose"
        return
    fi

    echo ""
}

CONTAINER_ENGINE=""
if command -v podman >/dev/null 2>&1; then
    CONTAINER_ENGINE="podman"
    echo -e "${GREEN}${OK} Podman installed: $(podman --version)${NC}"
elif command -v docker >/dev/null 2>&1; then
    CONTAINER_ENGINE="docker"
    echo -e "${YELLOW}${WARN} Podman not found, using Docker: $(docker --version)${NC}"
else
    echo -e "${RED}${ERR} Neither Podman nor Docker found${NC}"
    ISSUES=$((ISSUES + 1))
fi

COMPOSE_CMD="$(compose_cmd)"
if [[ -n "$COMPOSE_CMD" ]]; then
    echo -e "${GREEN}${OK} Compose available: ${COMPOSE_CMD}${NC}"
else
    echo -e "${RED}${ERR} No compose command found${NC}"
    ISSUES=$((ISSUES + 1))
fi

if [[ -f compose/.env ]]; then
    echo -e "${GREEN}${OK} compose/.env exists${NC}"
else
    echo -e "${RED}${ERR} compose/.env not found${NC}"
    ISSUES=$((ISSUES + 1))
fi

if [[ -f compose/docker-compose.yml ]]; then
    echo -e "${GREEN}${OK} compose/docker-compose.yml exists${NC}"
else
    echo -e "${RED}${ERR} compose/docker-compose.yml not found${NC}"
    ISSUES=$((ISSUES + 1))
fi

if [[ -n "$COMPOSE_CMD" ]] && [[ -f compose/docker-compose.yml ]]; then
    if (cd compose && $COMPOSE_CMD config >/dev/null 2>&1); then
        echo -e "${GREEN}${OK} Compose config is valid${NC}"
    else
        echo -e "${RED}${ERR} Failed to validate compose config${NC}"
        ISSUES=$((ISSUES + 1))
    fi
fi

if [[ -n "$CONTAINER_ENGINE" ]]; then
    if RUNNING=$($CONTAINER_ENGINE ps --format '{{.Names}}' 2>/dev/null | grep -c 'vipertrade' || true); then
        echo -e "${BLUE}${INFO} ViperTrade containers running: $RUNNING${NC}"
    else
        echo -e "${YELLOW}${WARN} Could not query containers (permission/socket?)${NC}"
    fi
fi

echo -e "${YELLOW}${INFO} Checking database connectivity...${NC}"
if command -v psql >/dev/null 2>&1; then
    echo -e "${GREEN}${OK} PostgreSQL client installed${NC}"
else
    echo -e "${YELLOW}${WARN} PostgreSQL client not installed (optional)${NC}"
fi

if command -v cargo >/dev/null 2>&1; then
    echo -e "${GREEN}${OK} Rust/Cargo installed: $(cargo --version)${NC}"
else
    echo -e "${YELLOW}${WARN} Rust/Cargo not installed (required for build)${NC}"
fi

DISK_USAGE=$(df -h . | awk 'NR==2 {gsub(/%/,"",$5); print $5}')
if [[ -n "$DISK_USAGE" ]] && [[ "$DISK_USAGE" -lt 80 ]]; then
    echo -e "${GREEN}${OK} Disk usage: ${DISK_USAGE}% used${NC}"
else
    echo -e "${RED}${ERR} Critical disk usage: ${DISK_USAGE:-unknown}% used${NC}"
    ISSUES=$((ISSUES + 1))
fi

if command -v free >/dev/null 2>&1; then
    MEM_AVAILABLE=$(free -m | awk '/^Mem:/ {print int($7/1024)}')
    if [[ -n "$MEM_AVAILABLE" ]] && [[ "$MEM_AVAILABLE" -ge 4 ]]; then
        echo -e "${GREEN}${OK} Available memory: ${MEM_AVAILABLE}GB${NC}"
    else
        echo -e "${YELLOW}${WARN} Low memory: ${MEM_AVAILABLE:-0}GB (recommended: 8GB+)${NC}"
    fi
fi

if git rev-parse --is-inside-work-tree > /dev/null 2>&1; then
    CHANGED=$(git status --porcelain | wc -l | tr -d ' ')
    if [[ "$CHANGED" -eq 0 ]]; then
        echo -e "${GREEN}${OK} Git working directory clean${NC}"
    else
        echo -e "${YELLOW}${WARN} Git working directory has $CHANGED modified file(s)${NC}"
    fi
else
    echo -e "${YELLOW}${WARN} Git repository not detected${NC}"
fi

echo ""
echo "================================================"
if [[ $ISSUES -eq 0 ]]; then
    echo -e "${GREEN}${OK} System healthy${NC}"
else
    echo -e "${RED}${ERR} $ISSUES issue(s) found${NC}"
fi
echo ""

exit $ISSUES