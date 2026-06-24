#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
SCRIPT_DIR="$ROOT_DIR/scripts"
cd "$ROOT_DIR"

. "$SCRIPT_DIR/lib/common.sh"

STRICT_DOCS="${CI_LOCAL_STRICT_DOCS:-0}"
SKIP_COMPOSE="${CI_LOCAL_SKIP_COMPOSE:-0}"
SKIP_EXECUTOR_DB="${CI_LOCAL_SKIP_EXECUTOR_DB:-0}"
SKIP_WEB_BUILD="${CI_LOCAL_SKIP_WEB_BUILD:-0}"
RUST_VERSION="${RUST_VERSION:-1.83}"
RUST_BUILDER_IMAGE="${RUST_BUILDER_IMAGE:-vipertrade-base-rust-builder:${RUST_VERSION}}"

show_help() {
   vt_print_header "ViperTrade - Local CI"
   echo ""
   echo "Usage:"
   echo "  ./scripts/ci-local.sh"
   echo ""
   echo "Environment:"
   echo "  CI_LOCAL_STRICT_DOCS=1        enable markdown lint"
   echo "  CI_LOCAL_SKIP_COMPOSE=1       skip compose config validation"
   echo "  CI_LOCAL_SKIP_EXECUTOR_DB=1   skip executor DB tests (requires PostgreSQL)"
   echo "  CI_LOCAL_SKIP_WEB_BUILD=1     skip web build (requires Node.js/Yarn)"
   echo "  EXECUTOR_TEST_DATABASE_URL    PostgreSQL connection URL for executor tests"
   echo "  PYO3_USE_ABI3_FORWARD_COMPATIBILITY"
   echo "                                host-side PyO3 compatibility flag (default: 1)"
   echo "  RUST_VERSION                    Rust builder version (default: 1.83)"
   echo "  RUST_BUILDER_IMAGE              Rust builder image used when the host toolchain is incomplete"
}

run_docs_lint() {
   local -a targets=(README.md CONTRIBUTING.md)
   while IFS= read -r file; do
      targets+=("$file")
   done < <(find docs -type f -name '*.md' | sort)

   # Pin to the exact version CI uses (.github/workflows/ci.yml). A host
   # markdownlint of a different version drifts from CI (e.g. 0.48 adds MD060),
   # which defeats the purpose of local parity — only trust it if it matches.
   local pinned="0.41.0"

   if command -v markdownlint >/dev/null 2>&1 \
      && [[ "$(markdownlint --version 2>/dev/null)" == "$pinned" ]]; then
      markdownlint "${targets[@]}"
      return
   fi

   if command -v npx >/dev/null 2>&1; then
      npx --yes "markdownlint-cli@${pinned}" "${targets[@]}"
      return
   fi

   vt_fail "Docs lint requires markdownlint ${pinned} or npx"
   return 1
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

# GitHub CI Rust Job tests
run_rust_check "Rust format check" cargo fmt --all -- --check
run_rust_check "Rust clippy (deny warnings)" cargo clippy --workspace --all-targets -- -D warnings
run_rust_check "Rust check" cargo check --workspace --locked
run_rust_check "Rust tests" cargo test --workspace --locked

# Compose config validation (GitHub CI: not in rust job, but in local parity)
if [[ "$SKIP_COMPOSE" != "1" ]] && vt_compose_available; then
   vt_step "Compose config validation"
   ./scripts/compose.sh -f compose/docker-compose.yml config >/dev/null
   vt_ok "Compose config validation"
else
   vt_warn "Compose validation skipped"
fi

# Make help validation
vt_step "Make help validation"
make help >/dev/null
vt_ok "Make help validation"

# Markdown lint (GitHub CI: docs job)
if [[ "$STRICT_DOCS" == "1" ]]; then
   vt_step "Markdown lint"
   run_docs_lint
   vt_ok "Markdown lint"
else
   vt_warn "Docs lint skipped (set CI_LOCAL_STRICT_DOCS=1 to enable)"
fi

# Web build (GitHub CI: web job)
if [[ "$SKIP_WEB_BUILD" == "1" ]]; then
   vt_warn "Web build skipped (CI_LOCAL_SKIP_WEB_BUILD=1)"
elif ! command -v yarn >/dev/null 2>&1; then
   vt_warn "Web build skipped (yarn not found)"
else
   vt_step "Web build"
   (cd services/web && yarn install --frozen-lockfile && yarn build)
   vt_ok "Web build"
fi

# Executor DB tests (GitHub CI: executor-db job)
if [[ "$SKIP_EXECUTOR_DB" == "1" ]]; then
   vt_warn "Executor DB tests skipped (CI_LOCAL_SKIP_EXECUTOR_DB=1)"
elif ! command -v psql >/dev/null 2>&1; then
   vt_warn "Executor DB tests skipped (psql not found)"
else
   EXECUTOR_TEST_DATABASE_URL="${EXECUTOR_TEST_DATABASE_URL:-postgres://viper:viper_secret_password@localhost:5432/vipertrade}"
   if pg_isready -d "$EXECUTOR_TEST_DATABASE_URL" >/dev/null 2>&1; then
      vt_step "Apply database schema"
      psql "$EXECUTOR_TEST_DATABASE_URL" -v ON_ERROR_STOP=1 -c "CREATE EXTENSION IF NOT EXISTS \"uuid-ossp\"" 2>/dev/null || true
      psql "$EXECUTOR_TEST_DATABASE_URL" -v ON_ERROR_STOP=1 -c "CREATE EXTENSION IF NOT EXISTS \"pgcrypto\"" 2>/dev/null || true
      psql "$EXECUTOR_TEST_DATABASE_URL" -v ON_ERROR_STOP=1 -f database/schema.sql 2>/dev/null || vt_warn "Some schema objects may require superuser (pg_stat_statements)"
      vt_ok "Apply database schema"
      vt_step "Executor DB tests"
      run_rust_check "Executor tests (with DB)" cargo test -p viper-executor --locked
      vt_ok "Executor DB tests"
   else
      vt_warn "Executor DB tests skipped (PostgreSQL not available at $EXECUTOR_TEST_DATABASE_URL)"
   fi
fi

vt_ok "Local CI passed"