#!/bin/bash
set -euo pipefail

GREEN="\033[0;32m"
YELLOW="\033[1;33m"
RED="\033[0;31m"
NC="\033[0m"

step() {
  echo -e "${GREEN}==>${NC} $1"
}

warn() {
  echo -e "${YELLOW}WARN:${NC} $1"
}

fail() {
  echo -e "${RED}ERROR:${NC} $1"
  exit 1
}

cd "$(dirname "$0")/.."

command -v cargo >/dev/null 2>&1 || fail "cargo not found"
command -v podman >/dev/null 2>&1 || fail "podman not found"

if podman compose version >/dev/null 2>&1; then
  COMPOSE_CMD="podman compose"
elif command -v podman-compose >/dev/null 2>&1; then
  COMPOSE_CMD="podman-compose"
else
  fail "podman compose/podman-compose not found"
fi

step "Rust format check"
cargo fmt --all -- --check

step "Rust clippy (deny warnings)"
cargo clippy --workspace --all-targets -- -D warnings

step "Rust tests"
cargo test --workspace --locked

if [[ -x scripts/validate-pipeline.sh ]]; then
  step "Tupa pipeline validation"
  ./scripts/validate-pipeline.sh
else
  warn "scripts/validate-pipeline.sh not found; skipping pipeline validation"
fi

step "Podman compose config validation"
$COMPOSE_CMD -f compose/docker-compose.yml config >/dev/null

if [[ "${CI_LOCAL_STRICT_DOCS:-0}" == "1" ]]; then
  if command -v markdownlint >/dev/null 2>&1; then
    step "Markdown lint"
    markdownlint "**/*.md"
  else
    fail "CI_LOCAL_STRICT_DOCS=1 but markdownlint is not installed"
  fi
else
  warn "Docs lint skipped (set CI_LOCAL_STRICT_DOCS=1 to enable)"
fi

step "Local CI passed"