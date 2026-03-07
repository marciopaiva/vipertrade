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

run_docs_lint() {
  local -a targets=(README.md docs/*.md VIPERTRADE_SPEC.md CONTRIBUTING.md)

  if command -v markdownlint >/dev/null 2>&1; then
    markdownlint "${targets[@]}"
    return
  fi

  if command -v npx >/dev/null 2>&1; then
    npx --yes markdownlint-cli@0.41.0 "${targets[@]}"
    return
  fi

  fail "Docs lint requires markdownlint or npx"
}

cd "$(dirname "$0")/.."

command -v cargo >/dev/null 2>&1 || fail "cargo not found"

if [[ "${CI_LOCAL_SKIP_COMPOSE:-0}" != "1" ]]; then
  command -v podman >/dev/null 2>&1 || fail "podman not found"
  [[ -x scripts/compose.sh ]] || fail "scripts/compose.sh not found or not executable"
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

if [[ "${CI_LOCAL_SKIP_COMPOSE:-0}" != "1" ]]; then
  step "Podman compose config validation"
  ./scripts/compose.sh config >/dev/null
else
  warn "Compose validation skipped (CI_LOCAL_SKIP_COMPOSE=1)"
fi

if [[ "${CI_LOCAL_STRICT_DOCS:-0}" == "1" ]]; then
  step "Markdown lint (strict)"
  run_docs_lint
else
  warn "Docs lint skipped (set CI_LOCAL_STRICT_DOCS=1 to enable)"
fi

step "Local CI passed"