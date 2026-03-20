#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
SCRIPT_DIR="$ROOT_DIR/scripts"
cd "$ROOT_DIR"

. "$SCRIPT_DIR/lib/common.sh"

STRICT_DOCS="${CI_LOCAL_STRICT_DOCS:-0}"
SKIP_COMPOSE="${CI_LOCAL_SKIP_COMPOSE:-0}"
SKIP_PIPELINE="${CI_LOCAL_SKIP_PIPELINE:-0}"
RUST_VERSION="${RUST_VERSION:-1.83}"
RUST_BUILDER_IMAGE="${RUST_BUILDER_IMAGE:-vipertrade-base-rust-builder:${RUST_VERSION}}"

show_help() {
  vt_print_header "ViperTrade - Local CI"
  echo ""
  echo "Usage:"
  echo "  ./scripts/ci-local.sh"
  echo ""
  echo "Environment:"
  echo "  CI_LOCAL_STRICT_DOCS=1    enable markdown lint"
  echo "  CI_LOCAL_SKIP_COMPOSE=1   skip compose config validation"
  echo "  CI_LOCAL_SKIP_PIPELINE=1  skip pipeline validation"
  echo "  PYO3_USE_ABI3_FORWARD_COMPATIBILITY"
  echo "                           host-side PyO3 compatibility flag (default: 1)"
  echo "  RUST_VERSION              Rust builder version (default: 1.83)"
  echo "  RUST_BUILDER_IMAGE        Rust builder image used when the host toolchain is incomplete"
}

run_docs_lint() {
  local -a targets=(README.md CONTRIBUTING.md)
  while IFS= read -r file; do
    targets+=("$file")
  done < <(find docs -type f -name '*.md' | sort)

  if command -v markdownlint >/dev/null 2>&1; then
    markdownlint "${targets[@]}"
    return
  fi

  if command -v npx >/dev/null 2>&1; then
    npx --yes markdownlint-cli@0.41.0 "${targets[@]}"
    return
  fi

  vt_fail "Docs lint requires markdownlint or npx"
  return 1
}

host_rust_ready() {
  vt_host_rust_ready
}

require_docker_compose() {
  docker compose version >/dev/null 2>&1
}

require_rust_builder_image() {
  vt_require_rust_builder_image "$RUST_BUILDER_IMAGE"
}

run_rust_in_docker() {
  local label="$1"
  shift

  vt_step "$label"
  docker run --rm \
    --user "$(id -u):$(id -g)" \
    -e CARGO_HOME=/tmp/cargo-home \
    -e CARGO_TARGET_DIR=/tmp/cargo-target \
    -e RUSTUP_HOME=/usr/local/rustup \
    -e HOME=/tmp \
    -v "$ROOT_DIR:/workspace" \
    -w /workspace \
    "$RUST_BUILDER_IMAGE" \
    "$@"
  vt_ok "$label"
}

run_rust_on_host() {
  local label="$1"
  shift
  vt_prepare_host_rust_env
  vt_step "$label"
  "$@"
  vt_ok "$label"
}

run_rust_check() {
  local label="$1"
  shift
  vt_run_rust_check "$label" "$RUST_BUILDER_IMAGE" "$@"
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" || "${1:-}" == "help" ]]; then
  show_help
  exit 0
fi

vt_print_header "ViperTrade - Local CI"

if [[ "$SKIP_COMPOSE" != "1" ]]; then
  require_docker_compose || { vt_fail "docker compose not found"; exit 1; }
  [[ -x scripts/compose.sh ]] || { vt_fail "scripts/compose.sh not found or not executable"; exit 1; }
fi

run_rust_check "Rust format check" cargo fmt --all -- --check
run_rust_check "Rust clippy (deny warnings)" cargo clippy --workspace --all-targets -- -D warnings
run_rust_check "Rust tests" cargo test --workspace --locked

if [[ "$SKIP_PIPELINE" == "1" ]]; then
  vt_warn "Pipeline validation skipped (CI_LOCAL_SKIP_PIPELINE=1)"
elif [[ -x scripts/validate-pipeline.sh ]]; then
  vt_step "Tupa pipeline validation"
  ./scripts/validate-pipeline.sh
  vt_ok "Tupa pipeline validation"
else
  vt_warn "scripts/validate-pipeline.sh not found; skipping pipeline validation"
fi

if [[ "$SKIP_COMPOSE" != "1" ]]; then
  vt_step "Compose config validation"
  ./scripts/compose.sh config >/dev/null
  vt_ok "Compose config validation"
else
  vt_warn "Compose validation skipped (CI_LOCAL_SKIP_COMPOSE=1)"
fi

vt_step "Make help validation"
make help >/dev/null
vt_ok "Make help validation"

if [[ "$STRICT_DOCS" == "1" ]]; then
  vt_step "Markdown lint (strict)"
  run_docs_lint
  vt_ok "Markdown lint (strict)"
else
  vt_warn "Docs lint skipped (set CI_LOCAL_STRICT_DOCS=1 to enable)"
fi

vt_ok "Local CI passed"
