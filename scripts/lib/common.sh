#!/usr/bin/env bash

VT_ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

VT_GREEN="\033[0;32m"
VT_RED="\033[0;31m"
VT_YELLOW="\033[1;33m"
VT_CYAN="\033[0;36m"
VT_BLUE="\033[0;34m"
VT_NC="\033[0m"

vt_root_dir() {
  printf '%s\n' "$VT_ROOT_DIR"
}

vt_cd_root() {
  cd "$VT_ROOT_DIR"
}

vt_print_header() {
  local title="$1"
  echo -e "${VT_GREEN}${title}${VT_NC}"
  echo "============================================"
}

vt_step() {
  echo -e "${VT_YELLOW}→${VT_NC} $1"
}

vt_ok() {
  echo -e "${VT_GREEN}✓${VT_NC} $1"
}

vt_warn() {
  echo -e "${VT_YELLOW}!${VT_NC} $1"
}

vt_fail() {
  echo -e "${VT_RED}✗${VT_NC} $1" >&2
}

vt_info() {
  echo -e "${VT_CYAN}$1${VT_NC}"
}

vt_require_cmd() {
  local command_name="$1"
  if ! command -v "$command_name" >/dev/null 2>&1; then
    vt_fail "$command_name not found"
    return 1
  fi
}
