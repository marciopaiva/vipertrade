#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
SCRIPT_DIR="$ROOT_DIR/scripts"
cd "$ROOT_DIR"

. "$SCRIPT_DIR/lib/common.sh"
vt_prepare_host_rust_env

MODE="${1:-all}"
STRICT_DOCS="${CI_LOCAL_STRICT_DOCS:-0}"
SKIP_COMPOSE="${CI_LOCAL_SKIP_COMPOSE:-0}"
SKIP_PIPELINE="${CI_LOCAL_SKIP_PIPELINE:-0}"
RUST_VERSION="${RUST_VERSION:-1.83}"
RUST_BUILDER_IMAGE="${RUST_BUILDER_IMAGE:-vipertrade-base-rust-builder:${RUST_VERSION}}"

PASS_COUNT=0
FAIL_COUNT=0
WARN_COUNT=0

print_header() {
  vt_print_header "ViperTrade - Workspace Validation"
}

print_step() {
  vt_step "$1"
}

print_ok() {
  PASS_COUNT=$((PASS_COUNT + 1))
  vt_ok "$1"
}

print_warn() {
  WARN_COUNT=$((WARN_COUNT + 1))
  vt_warn "$1"
}

print_fail() {
  FAIL_COUNT=$((FAIL_COUNT + 1))
  vt_fail "$1"
}

run_check() {
  local label="$1"
  shift
  print_step "$label"
  if "$@"; then
    print_ok "$label"
  else
    print_fail "$label"
  fi
}

run_rust_check() {
  local label="$1"
  shift
  if vt_run_rust_check "$label" "$RUST_BUILDER_IMAGE" "$@"; then
    PASS_COUNT=$((PASS_COUNT + 1))
  else
    FAIL_COUNT=$((FAIL_COUNT + 1))
  fi
}

show_help() {
  print_header
  echo ""
  echo "Usage: $0 [all|quick|ci]"
  echo ""
  echo "Modes:"
  echo "  all    - fmt, clippy, tests, ci-local, and supporting checks"
  echo "  quick  - fmt, clippy, and tests"
  echo "  ci     - run scripts/ci-local.sh"
  echo ""
  echo "Useful variables:"
  echo "  CI_LOCAL_STRICT_DOCS=1  enable markdown lint"
  echo "  CI_LOCAL_SKIP_COMPOSE=1 skip compose config validation"
  echo "  CI_LOCAL_SKIP_PIPELINE=1 skip validate-pipeline"
  echo "  PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 keep host-side PyO3 checks compatible"
  echo "  RUST_BUILDER_IMAGE      builder image used when rustfmt/clippy are unavailable"
}

require_command() {
  local command_name="$1"
  if ! command -v "$command_name" >/dev/null 2>&1; then
    print_fail "$command_name not found"
    return 1
  fi
  return 0
}

run_quick_suite() {
  run_rust_check "Rust format check" cargo fmt --all -- --check
  run_rust_check "Rust clippy" cargo clippy --workspace --all-targets -- -D warnings
  run_rust_check "Rust tests" cargo test --workspace --locked
}

run_full_suite() {
  run_quick_suite

  if [[ -x ./scripts/security-check.sh ]]; then
    run_check "Security checks" ./scripts/security-check.sh
  else
    print_warn "scripts/security-check.sh not found; skipping"
  fi

  if [[ -x ./scripts/health-check.sh ]]; then
    run_check "Health check script (help)" ./scripts/health-check.sh --help >/dev/null
  else
    print_warn "scripts/health-check.sh not found; skipping"
  fi

  if [[ -x ./scripts/ci-local.sh ]]; then
    run_check "CI local" env CI_LOCAL_STRICT_DOCS="$STRICT_DOCS" CI_LOCAL_SKIP_COMPOSE="$SKIP_COMPOSE" CI_LOCAL_SKIP_PIPELINE="$SKIP_PIPELINE" ./scripts/ci-local.sh
  else
    print_fail "scripts/ci-local.sh not found"
  fi
}

print_summary() {
  echo ""
  echo -e "${VT_CYAN}Summary:${VT_NC} PASS=$PASS_COUNT WARN=$WARN_COUNT FAIL=$FAIL_COUNT"
}

main() {
  if [[ "$MODE" == "help" || "$MODE" == "-h" || "$MODE" == "--help" ]]; then
    show_help
    exit 0
  fi

  print_header
  require_command cargo || exit 1

  case "$MODE" in
    quick)
      run_quick_suite
      ;;
    ci)
      if [[ -x ./scripts/ci-local.sh ]]; then
        run_check "CI local" env CI_LOCAL_STRICT_DOCS="$STRICT_DOCS" CI_LOCAL_SKIP_COMPOSE="$SKIP_COMPOSE" CI_LOCAL_SKIP_PIPELINE="$SKIP_PIPELINE" ./scripts/ci-local.sh
      else
        print_fail "scripts/ci-local.sh not found"
      fi
      ;;
    all)
      run_full_suite
      ;;
    *)
      print_fail "Unrecognized mode '$MODE'"
      show_help
      exit 1
      ;;
  esac

  print_summary
  [[ "$FAIL_COUNT" -eq 0 ]]
}

main "$@"
