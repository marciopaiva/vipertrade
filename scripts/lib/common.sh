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

vt_prepare_host_rust_env() {
  # Fedora 43 ships Python 3.14 while PyO3 0.21.x officially supports up to 3.12.
  # Keep host-side validation usable until the PyO3 dependency is upgraded.
  export PYO3_USE_ABI3_FORWARD_COMPATIBILITY="${PYO3_USE_ABI3_FORWARD_COMPATIBILITY:-1}"
}

vt_host_rust_ready() {
  command -v cargo >/dev/null 2>&1 \
    && cargo fmt --version >/dev/null 2>&1 \
    && cargo clippy --version >/dev/null 2>&1
}

vt_require_rust_builder_image() {
  local image_name="$1"
  docker image inspect "$image_name" >/dev/null 2>&1
}

vt_run_rust_check() {
  local label="$1"
  local image_name="$2"
  shift 2

  if vt_host_rust_ready; then
    vt_prepare_host_rust_env
    vt_step "$label"
    "$@"
    vt_ok "$label"
    return 0
  fi

  if ! command -v docker >/dev/null 2>&1; then
    vt_fail "cargo fmt/clippy are unavailable on the host and docker was not found"
    return 1
  fi

  if ! vt_require_rust_builder_image "$image_name"; then
    vt_fail "image $image_name not found; run make build-base-images"
    return 1
  fi

  vt_warn "rustfmt/clippy are unavailable on the host; using $image_name"
  vt_step "$label"
  docker run --rm \
    --user "$(id -u):$(id -g)" \
    -e CARGO_HOME=/tmp/cargo-home \
    -e CARGO_TARGET_DIR=/tmp/cargo-target \
    -e RUSTUP_HOME=/usr/local/rustup \
    -e HOME=/tmp \
    -v "$VT_ROOT_DIR:/workspace" \
    -w /workspace \
    "$image_name" \
    "$@"
  vt_ok "$label"
}
