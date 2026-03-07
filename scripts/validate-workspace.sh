#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

TIMESTAMP="$(date +%Y%m%d-%H%M%S)"
REPORT_FILE="logs/workspace-validation-${TIMESTAMP}.md"

mkdir -p logs

PASS=0
FAIL=0
WARN=0

run_step() {
  local name="$1"
  local cmd="$2"

  {
    echo "## ${name}"
    echo
    echo '```bash'
    echo "$cmd"
    echo '```'
    echo
  } >> "$REPORT_FILE"

  if bash -lc "$cmd" >> "$REPORT_FILE" 2>&1; then
    echo "- status: PASS" >> "$REPORT_FILE"
    echo >> "$REPORT_FILE"
    PASS=$((PASS + 1))
  else
    echo "- status: FAIL" >> "$REPORT_FILE"
    echo >> "$REPORT_FILE"
    FAIL=$((FAIL + 1))
  fi
}

run_optional_step() {
  local name="$1"
  local cmd="$2"

  {
    echo "## ${name}"
    echo
    echo '```bash'
    echo "$cmd"
    echo '```'
    echo
  } >> "$REPORT_FILE"

  if bash -lc "$cmd" >> "$REPORT_FILE" 2>&1; then
    echo "- status: PASS" >> "$REPORT_FILE"
    echo >> "$REPORT_FILE"
    PASS=$((PASS + 1))
  else
    echo "- status: WARN" >> "$REPORT_FILE"
    echo >> "$REPORT_FILE"
    WARN=$((WARN + 1))
  fi
}

cat > "$REPORT_FILE" <<EOF
# Workspace Validation Report

- generated_at: $(date -Iseconds)
- workspace: ${ROOT_DIR}

EOF

run_step "Security Check" "./scripts/security-check.sh"
run_optional_step "Health Check" "./scripts/health-check.sh"
run_step "Rust Format" "cargo fmt --all -- --check"
run_step "Rust Clippy" "cargo clippy --workspace --all-targets -- -D warnings"
run_step "Rust Tests" "cargo test --workspace --locked"
run_step "Local CI Script" "CI_LOCAL_STRICT_DOCS=1 ./scripts/ci-local.sh"

{
  echo "## Summary"
  echo
  echo "- pass: ${PASS}"
  echo "- warn: ${WARN}"
  echo "- fail: ${FAIL}"
  echo
} >> "$REPORT_FILE"

echo "Validation report: ${REPORT_FILE}"

if [[ "$FAIL" -gt 0 ]]; then
  exit 1
fi
