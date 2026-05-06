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

vt_container_engine() {
  if [[ -n "${VT_CONTAINER_ENGINE:-}" ]]; then
    printf '%s\n' "$VT_CONTAINER_ENGINE"
    return 0
  fi

  if [[ -n "${CONTAINER_ENGINE:-}" ]]; then
    printf '%s\n' "$CONTAINER_ENGINE"
    return 0
  fi

  if command -v podman >/dev/null 2>&1 && vt_podman_remote_enabled; then
    printf 'podman\n'
    return 0
  fi

  if command -v docker >/dev/null 2>&1; then
    printf 'docker\n'
    return 0
  fi

  if command -v podman >/dev/null 2>&1; then
    printf 'podman\n'
    return 0
  fi

  vt_fail "container engine not found (install docker or podman)"
  return 1
}

vt_container() {
  local engine
  engine="$(vt_container_engine)" || return 1
  if [[ "$engine" == "podman" ]]; then
    if vt_podman_remote_enabled; then
      "$engine" --remote "$@"
      return
    fi

    local -a podman_args=()
    read -r -a podman_args <<< "${VT_PODMAN_ARGS:---cgroup-manager=cgroupfs}"
    "$engine" "${podman_args[@]}" "$@"
    return
  fi
  "$engine" "$@"
}

vt_podman_remote_enabled() {
  case "${VT_PODMAN_REMOTE:-auto}" in
    1|true|TRUE|yes|YES)
      return 0
      ;;
    0|false|FALSE|no|NO)
      return 1
      ;;
  esac

  podman system connection list --format '{{.Default}}' 2>/dev/null | grep -qi '^true$'
}

vt_container_available() {
  vt_container_engine >/dev/null
}

vt_compose_available() {
  local engine
  engine="$(vt_container_engine 2>/dev/null || true)"

  if [[ -n "${VT_COMPOSE_COMMAND:-}" ]]; then
    return 0
  fi

  if [[ "$engine" == "docker" ]] && command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then
    return 0
  fi

  if [[ "$engine" != "podman" ]]; then
    return 1
  fi

  if command -v podman-compose >/dev/null 2>&1; then
    return 0
  fi

  if command -v podman >/dev/null 2>&1; then
    if vt_podman_remote_enabled && podman --remote compose version >/dev/null 2>&1; then
      return 0
    fi

    if podman --cgroup-manager=cgroupfs compose version >/dev/null 2>&1; then
      return 0
    fi
  fi

  return 1
}

vt_podman_compose_args() {
  if vt_podman_remote_enabled; then
    printf '%s\n' "${VT_PODMAN_REMOTE_ARGS:-}"
    return
  fi

  printf '%s\n' "${VT_PODMAN_ARGS:---cgroup-manager=cgroupfs}"
}

vt_compose_label() {
  local engine
  engine="$(vt_container_engine 2>/dev/null || true)"

  if [[ -n "${VT_COMPOSE_COMMAND:-}" ]]; then
    printf '%s\n' "$VT_COMPOSE_COMMAND"
  elif [[ "$engine" == "docker" ]] && command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then
    printf 'docker compose\n'
  elif [[ "$engine" == "podman" ]] && command -v podman-compose >/dev/null 2>&1; then
    if vt_podman_remote_enabled; then
      printf 'podman-compose (podman --remote)\n'
    else
      printf 'podman-compose\n'
    fi
  elif [[ "$engine" == "podman" ]] && command -v podman >/dev/null 2>&1; then
    if vt_podman_remote_enabled && podman --remote compose version >/dev/null 2>&1; then
      printf 'podman --remote compose\n'
    elif podman --cgroup-manager=cgroupfs compose version >/dev/null 2>&1; then
      printf 'podman compose\n'
    else
      printf 'compose unavailable\n'
    fi
  else
    printf 'compose unavailable\n'
  fi
}

vt_compose() {
  local engine
  engine="$(vt_container_engine)" || return 1

  if [[ -n "${VT_COMPOSE_COMMAND:-}" ]]; then
    local -a command_parts=()
    read -r -a command_parts <<< "$VT_COMPOSE_COMMAND"
    "${command_parts[@]}" "$@"
    return 0
  fi

  if [[ "$engine" == "docker" ]] && command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then
    docker compose "$@"
    return
  fi

  if [[ "$engine" != "podman" ]]; then
    vt_fail "compose runtime not found for container engine: $engine"
    return 1
  fi

  if command -v podman-compose >/dev/null 2>&1; then
    if vt_podman_remote_enabled; then
      podman-compose --podman-path "$VT_ROOT_DIR/scripts/podman-remote.sh" "$@"
      return
    fi

    podman-compose --podman-args="$(vt_podman_compose_args)" "$@"
    return
  fi

  if command -v podman >/dev/null 2>&1; then
    if vt_podman_remote_enabled && podman --remote compose version >/dev/null 2>&1; then
      podman --remote compose "$@"
      return
    fi

    local -a podman_args=()
    read -r -a podman_args <<< "${VT_PODMAN_ARGS:---cgroup-manager=cgroupfs}"
    podman "${podman_args[@]}" compose "$@"
    return
  fi

  vt_fail "compose runtime not found (docker compose, podman compose, or podman-compose)"
  return 1
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
  vt_container image inspect "$image_name" >/dev/null 2>&1
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

  if ! vt_container_available; then
    vt_fail "cargo fmt/clippy are unavailable on the host and no container engine was found"
    return 1
  fi

  if ! vt_require_rust_builder_image "$image_name"; then
    vt_fail "image $image_name not found; run make build-base-images"
    return 1
  fi

  vt_warn "rustfmt/clippy are unavailable on the host; using $image_name"
  vt_step "$label"
  vt_container run --rm \
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

# WSL detection
vt_is_wsl() {
  grep -qi microsoft /proc/version 2>/dev/null
}

# Suggest registry based on environment (WSL vs native)
vt_suggest_registry() {
  local test_registry="${1:-localhost:5001}"

  if vt_is_wsl; then
    # In WSL, check if localhost:5001 is reachable; if not, try host.docker.internal
    if curl -s --max-time 2 "http://$test_registry/v2/" >/dev/null 2>&1; then
      echo "$test_registry"
      return 0
    fi

    if curl -s --max-time 2 "http://host.docker.internal:5001/v2/" >/dev/null 2>&1; then
      echo "host.docker.internal:5001"
      return 0
    fi

    # Fallback to the original suggestion
    echo "$test_registry"
    return 1
  fi

  echo "$test_registry"
}

# Check if local registry is accessible
vt_registry_available() {
  local registry="${1:-$(vt_suggest_registry)}"
  curl -s --max-time 3 "http://$registry/v2/" >/dev/null 2>&1
}
