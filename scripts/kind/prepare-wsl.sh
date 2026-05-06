#!/usr/bin/env bash
# ViperTrade - Prepare WSL environment for Kind + Podman
# Ensures local registry is running on the 'kind' network

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../lib/common.sh"

vt_cd_root

LOCAL_REGISTRY_NAME="${LOCAL_REGISTRY_NAME:-kind-registry}"
LOCAL_REGISTRY_PORT="${LOCAL_REGISTRY_PORT:-5001}"

vt_require_cmd podman
vt_require_cmd kubectl

vt_print_header "Prepare WSL for ViperTrade Kind"

# Verify we're on WSL (optional but helpful)
if vt_is_wsl; then
  vt_info "WSL environment detected"
else
  vt_warn "Not running on WSL; this script is optimized for WSL2 but will continue"
fi

# Ensure the 'kind' network exists (created by setup-k8s-wsl2.sh)
if ! podman network exists kind 2>/dev/null; then
  vt_fail "Podman network 'kind' not found. Run setup-k8s-wsl2/setup.sh first."
  exit 1
fi
vt_ok "Podman network 'kind' exists"

# Ensure the Kind cluster exists
if ! kind get clusters 2>/dev/null | grep -q "^dev$"; then
  vt_fail "Kind cluster 'dev' not found. Run setup-k8s-wsl2/setup.sh or create a cluster."
  exit 1
fi
vt_ok "Kind cluster 'dev' exists"

# Start or create local registry on the 'kind' network
if podman container exists "$LOCAL_REGISTRY_NAME" 2>/dev/null; then
  vt_info "Registry container '$LOCAL_REGISTRY_NAME' exists"

  # Check if it's on the kind network
  networks="$(podman inspect --format '{{range $name, $_ := .NetworkSettings.Networks}}{{$name}} {{end}}' "$LOCAL_REGISTRY_NAME" 2>/dev/null || true)"

  if [[ " $networks " == *" kind "* ]]; then
    vt_info "Registry is already on 'kind' network"
    if ! podman container inspect "$LOCAL_REGISTRY_NAME" --format '{{.State.Status}}' 2>/dev/null | grep -q "^running$"; then
      vt_step "Starting registry container"
      podman start "$LOCAL_REGISTRY_NAME"
    fi
    vt_ok "Local registry is running on kind network"
  else
    vt_warn "Registry is not on 'kind' network; recreating container on kind network"
    podman rm -f "$LOCAL_REGISTRY_NAME" || true

    vt_step "Creating registry on 'kind' network"
    podman run -d \
      --name "$LOCAL_REGISTRY_NAME" \
      --network kind \
      -p "127.0.0.1:${LOCAL_REGISTRY_PORT}:5000" \
      --restart=always \
      docker.io/library/registry:2 || {
        vt_fail "Failed to create registry container"
        exit 1
      }
    vt_ok "Local registry created on kind network"
  fi
else
  vt_info "Registry container not found; creating new one"

  vt_step "Pulling registry image"
  vt_container pull docker.io/library/registry:2 || {
    vt_fail "Failed to pull registry image"
    exit 1
  }

  vt_step "Creating registry container on 'kind' network"
  podman run -d \
    --name "$LOCAL_REGISTRY_NAME" \
    --network kind \
    -p "127.0.0.1:${LOCAL_REGISTRY_PORT}:5000" \
    --restart=always \
    docker.io/library/registry:2 || {
      vt_fail "Failed to create registry container"
      exit 1
    }
  vt_ok "Local registry created on kind network"
fi

# Verify registry is responding
vt_step "Verifying registry health"
if curl -s --max-time 3 "http://localhost:${LOCAL_REGISTRY_PORT}/v2/" >/dev/null 2>&1; then
  vt_ok "Registry responding at http://localhost:${LOCAL_REGISTRY_PORT}"
else
  vt_warn "Registry not responding; it may still be starting up"
  vt_info "Check with: curl http://localhost:${LOCAL_REGISTRY_PORT}/v2/"
fi

# Show kind network info
echo ""
vt_info "Podman network 'kind' containers:"
podman network inspect kind -f '{{range .Containers}}{{.Name}} {{end}}' 2>/dev/null || true

echo ""
vt_ok "WSL environment prepared"
echo ""
vt_info "Next steps:"
vt_info "  1. Build images:  make kind-build-images"
vt_info "  2. Deploy:        make kind-deploy"
vt_info "  3. Health check:  ./scripts/kind/health-check.sh"
vt_info "  4. Status:        make kind-status"
echo ""
vt_info "Note: If the registry is not accessible from the host, set:"
vt_info "  export KIND_REGISTRY=host.docker.internal:5001"
vt_info "before running 'make kind-build-images'"
