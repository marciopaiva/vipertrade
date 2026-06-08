#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/../lib/common.sh"

vt_cd_root

KIND_CONTEXT="${KIND_CONTEXT:-kind-dev}"
KIND_NAMESPACE="${KIND_NAMESPACE:-vipertrade}"
KIND_REGISTRY="${KIND_REGISTRY:-$(vt_suggest_registry)}"
IMAGE_TAG="${IMAGE_TAG:-dev}"

vt_require_cmd kubectl

vt_print_header "Deploy ViperTrade to Kind"
vt_info "Context: $KIND_CONTEXT | Namespace: $KIND_NAMESPACE | Registry: $KIND_REGISTRY | Image tag: $IMAGE_TAG"

# Verify images in registry
vt_step "Verifying images in registry"
images=(vipertrade vipertrade-postgres vipertrade-web)

for img in "${images[@]}"; do
  if ! curl -s "http://$KIND_REGISTRY/v2/$img/tags/list" 2>/dev/null | grep -q "$IMAGE_TAG"; then
    vt_fail "Image $KIND_REGISTRY/$img:$IMAGE_TAG not found. Run 'make kind-build-images' first."
    exit 1
  fi
done
vt_ok "All images present"

# Deploy
vt_step "Applying k8s/kind manifests"
kubectl --context "$KIND_CONTEXT" apply -k k8s/kind

vt_step "Waiting for infrastructure"
kubectl --context "$KIND_CONTEXT" -n "$KIND_NAMESPACE" rollout status deployment/postgres --timeout=180s
kubectl --context "$KIND_CONTEXT" -n "$KIND_NAMESPACE" rollout status deployment/redis --timeout=120s

vt_step "Waiting for application deployments"
for deployment in market-data analytics strategy executor monitor api ai-analyst web; do
  kubectl --context "$KIND_CONTEXT" -n "$KIND_NAMESPACE" rollout status "deployment/$deployment" --timeout=240s
done

vt_ok "ViperTrade is reachable at http://localhost:8080 (api) and http://localhost:30080 (web)"
vt_info "Next steps: make kind-status | make health"

