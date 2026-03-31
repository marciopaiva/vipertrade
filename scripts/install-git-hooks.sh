#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
HOOKS_DIR="$ROOT_DIR/.githooks"

if [[ ! -d "$HOOKS_DIR" ]]; then
  echo "Missing hooks directory: $HOOKS_DIR" >&2
  exit 1
fi

chmod +x "$HOOKS_DIR"/*
git -C "$ROOT_DIR" config core.hooksPath .githooks

echo "Git hooks installed from .githooks"
echo "pre-push will now run: make validate-ci"
