#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")/.."
git config core.hooksPath .githooks
echo "OK: git hooks path configured to .githooks"