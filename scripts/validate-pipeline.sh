#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$SCRIPT_DIR/lib/common.sh"

PIPELINE_FILE="${PIPELINE_FILE:-config/strategies/viper_smart_copy.tp}"
CHECK_LOG="${CHECK_LOG:-/tmp/tupa_check.log}"
AST_OUT="${AST_OUT:-/tmp/viper_smart_copy.ast.json}"
TUPA_BIN="${TUPA_BIN:-tupa}"
TUPA_DOCKER_IMAGE="${TUPA_DOCKER_IMAGE:-compose-strategy:latest}"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

show_help() {
  vt_print_header "ViperTrade - Pipeline Validation"
  echo ""
  echo "Usage:"
  echo "  ./scripts/validate-pipeline.sh"
  echo ""
  echo "Environment:"
  echo "  PIPELINE_FILE       Pipeline source file"
  echo "  TUPA_BIN            Tupa binary name/path (default: tupa)"
  echo "  TUPA_DOCKER_IMAGE   Docker image used when Tupa is not installed locally"
  echo "  CHECK_LOG           Temporary check log path"
  echo "  AST_OUT             Output path for the generated AST JSON"
}

run_tupa() {
  if command -v "$TUPA_BIN" >/dev/null 2>&1; then
    "$TUPA_BIN" "$@"
    return
  fi

  if ! command -v docker >/dev/null 2>&1; then
    vt_fail "'$TUPA_BIN' not found and docker is unavailable"
    return 1
  fi

  if ! docker image inspect "$TUPA_DOCKER_IMAGE" >/dev/null 2>&1; then
    vt_fail "'$TUPA_BIN' not found and image $TUPA_DOCKER_IMAGE is unavailable"
    vt_warn "start the stack or build the strategy image before validation"
    return 1
  fi

  docker run --rm \
    --user "$(id -u):$(id -g)" \
    -e HOME=/tmp \
    -v "$ROOT_DIR:/workspace" \
    -w /workspace \
    --entrypoint tupa \
    "$TUPA_DOCKER_IMAGE" \
    "$@"
}

main() {
  if [[ "${1:-}" == "-h" || "${1:-}" == "--help" || "${1:-}" == "help" ]]; then
    show_help
    exit 0
  fi

  vt_cd_root
  vt_print_header "ViperTrade - Pipeline Validation"

  if [[ ! -f "$PIPELINE_FILE" ]]; then
    vt_fail "pipeline file not found: $PIPELINE_FILE"
    exit 1
  fi
  vt_ok "pipeline file found"

  if command -v "$TUPA_BIN" >/dev/null 2>&1; then
    vt_info "Using local Tupa CLI: $(command -v "$TUPA_BIN")"
  else
    vt_warn "'$TUPA_BIN' not found on the host; using $TUPA_DOCKER_IMAGE"
  fi

  vt_step "Checking syntax and types"
  if run_tupa check "$PIPELINE_FILE" 2>&1 | tee "$CHECK_LOG"; then
    vt_ok "syntax/type check passed"
  else
    vt_fail "syntax/type check failed"
    exit 1
  fi

  vt_step "Generating AST JSON"
  if run_tupa parse --format json "$PIPELINE_FILE" > "$AST_OUT"; then
    vt_ok "AST generated"
  else
    vt_fail "failed to generate AST"
    exit 1
  fi

  if [[ ! -s "$AST_OUT" ]]; then
    vt_fail "generated AST is empty"
    exit 1
  fi
  vt_ok "AST generated with content"

  if command -v jq >/dev/null 2>&1; then
    if jq empty "$AST_OUT" >/dev/null 2>&1; then
      vt_ok "AST JSON is valid"
    else
      vt_fail "AST JSON is invalid"
      exit 1
    fi

    if jq -e '.items and (.items | length > 0)' "$AST_OUT" >/dev/null 2>&1; then
      vt_ok "AST contains items"
    else
      vt_fail "AST has no items"
      exit 1
    fi
  else
    vt_warn "jq not found; skipping structural JSON validation"
  fi

  rm -f "$CHECK_LOG"
  echo ""
  vt_ok "pipeline validation complete"
}

main "$@"
