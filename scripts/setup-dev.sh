#!/usr/bin/env bash
set -euo pipefail

# setup-dev.sh — Installs tools required for ViperTrade development.
#
# Currently installs cargo-tupa (TupaLang CLI) by building it from the sibling
# tupalang/ workspace, if it isn't already on $PATH.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TUPALANG_DIR="$(cd "$WORKSPACE_ROOT/../tupalang" && pwd)"

echo "=== ViperTrade Dev Setup ==="

# ── cargo-tupa ────────────────────────────────────────────────────────────────
if command -v cargo-tupa &>/dev/null; then
    echo "  [ok]  cargo-tupa already on PATH ($(which cargo-tupa))"
else
    echo "  [···] Building cargo-tupa from $TUPALANG_DIR …"
    if [[ ! -d "$TUPALANG_DIR" ]]; then
        echo "  [err] Sibling workspace not found at $TUPALANG_DIR"
        echo "        Expected layout:"
        echo "          tupa/"
        echo "            ├── vipertrade/"
        echo "            └── tupalang/"
        exit 1
    fi
    cargo install --path "$TUPALANG_DIR/crates/cargo-tupa" --root "$HOME/.cargo" 2>&1
    echo "  [ok]  cargo-tupa installed to $(which cargo-tupa)"
fi

echo "=== Done ==="
