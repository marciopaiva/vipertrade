#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../lib/common.sh"

vt_cd_root

KIND_CONTEXT="${KIND_CONTEXT:-kind-dev}"
KIND_NAMESPACE="${KIND_NAMESPACE:-vipertrade}"
KIND_REGISTRY="${KIND_REGISTRY:-$(vt_suggest_registry)}"
IMAGE_TAG="${IMAGE_TAG:-dev}"

vt_require_cmd kubectl

vt_print_header "Deploy ViperTrade to Kind"
vt_info "Context: $KIND_CONTEXT"
vt_info "Namespace: $KIND_NAMESPACE"
vt_info "Registry: $KIND_REGISTRY"
vt_info "Image tag: $IMAGE_TAG"

# Pre-flight: verify images exist in registry
vt_step "Verifying images in registry $KIND_REGISTRY"
images=(
  vipertrade-market-data
  vipertrade-analytics
  vipertrade-strategy
  vipertrade-executor
  vipertrade-monitor
  vipertrade-backtest
  vipertrade-api
  vipertrade-ai-analyst
  vipertrade-web
)

for img in "${images[@]}"; do
  full="$KIND_REGISTRY/$img:$IMAGE_TAG"
  if ! vt_registry_available "$KIND_REGISTRY"; then
    vt_fail "Registry $KIND_REGISTRY not accessible"
    exit 1
  fi

  # Attempt to query the registry for the image tag
  if ! curl -s "http://$KIND_REGISTRY/v2/$img/tags/list" 2>/dev/null | grep -q "$IMAGE_TAG"; then
    vt_fail "Image $full not found in registry. Run 'make kind-build-images' first."
    exit 1
  fi
done
vt_ok "All images present in registry"

# Apply K8s manifests
vt_step "Applying k8s/kind to context $KIND_CONTEXT"
if ! kubectl --context "$KIND_CONTEXT" apply -k k8s/kind; then
  vt_fail "Failed to apply K8s manifests"
  exit 1
fi

vt_step "Waiting for infrastructure"
kubectl --context "$KIND_CONTEXT" -n "$KIND_NAMESPACE" rollout status deployment/postgres --timeout=180s
kubectl --context "$KIND_CONTEXT" -n "$KIND_NAMESPACE" rollout status deployment/redis --timeout=120s

vt_step "Waiting for application deployments"
for deployment in market-data analytics strategy executor monitor backtest api ai-analyst web; do
  kubectl --context "$KIND_CONTEXT" -n "$KIND_NAMESPACE" rollout status "deployment/$deployment" --timeout=240s
done

vt_ok "ViperTrade is reachable at http://localhost:8080 (api) and http://localhost:30080 (web)"

# Show helpful next steps
vt_info ""
vt_info "Next steps:"
vt_info "  - Health:    ./scripts/kind/health-check.sh"
vt_info "  - Status:    make kind-status"
vt_info "  - Logs:      kubectl logs -n $KIND_NAMESPACE -f deployment/strategy"
vt_info "  - Dashboard: k9s (if installed)"

