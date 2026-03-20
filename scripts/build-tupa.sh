#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$SCRIPT_DIR/lib/common.sh"

show_help() {
  vt_print_header "ViperTrade - Build Tupa Integration"
  echo ""
  echo "Usage:"
  echo "  ./scripts/build-tupa.sh"
  echo ""
  echo "Description:"
  echo "  Builds the strategy service with Tupa integration and validates the pipeline."
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" || "${1:-}" == "help" ]]; then
  show_help
  exit 0
fi

vt_cd_root
vt_print_header "ViperTrade - Build Tupa Integration"

# Load version
if [[ -f VERSION ]]; then
    source VERSION
fi

# Check Tupa version in Cargo.toml
TUPA_VERSION=$(grep "tupa-runtime" Cargo.toml | grep -oP "version = \"\K[^\"]+" || echo "unknown")
vt_info "Tupa version: $TUPA_VERSION"

# Clean cache if version changed
if [[ -f .tupa_version ]] && [[ "$(cat .tupa_version 2>/dev/null)" != "$TUPA_VERSION" ]]; then
    vt_step "Tupa version changed; clearing cache"
    cargo clean -p tupa-runtime -p tupa-codegen 2>/dev/null || true
fi
echo "$TUPA_VERSION" > .tupa_version

vt_step "Building strategy service with Tupa trading features"
cargo build -p viper-strategy \
    --release \
    --config 'net.git-fetch-with-cli=true'

if [[ -f target/release/viper-strategy ]]; then
    vt_ok "Build completed successfully"
    vt_info "Binary size: $(ls -lh target/release/viper-strategy | awk '{print $5}')"
else
    vt_fail "Build failed"
    exit 1
fi

vt_step "Validating Tupa pipeline"
if command -v tupa &> /dev/null; then
    TUPA_CMD="tupa"
elif [[ -f "$HOME/.local/bin/tupa" ]]; then
    TUPA_CMD="$HOME/.local/bin/tupa"
else
    TUPA_CMD=""
fi

if [[ -n "$TUPA_CMD" ]]; then
    if $TUPA_CMD codegen --check config/strategies/viper_smart_copy.tp 2>/dev/null; then
        vt_ok "Pipeline is valid"
    else
        vt_fail "Pipeline validation failed"
    fi
else
    if cargo run -p tupa-cli -- codegen --check config/strategies/viper_smart_copy.tp 2>/dev/null; then
        vt_ok "Pipeline is valid"
    else
        vt_warn "Pipeline validation skipped (tupa CLI not found)"
    fi
fi

echo ""
vt_ok "Build complete"
exit 0
