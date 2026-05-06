#!/usr/bin/env bash
# ViperTrade - Update Kubernetes Secret from .env file
# Syncs secrets from compose/.env to the K8s cluster

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../lib/common.sh"

vt_cd_root

KIND_CONTEXT="${KIND_CONTEXT:-kind-dev}"
KIND_NAMESPACE="${KIND_NAMESPACE:-vipertrade}"
ENV_FILE="${ENV_FILE:-compose/.env}"
SECRET_NAME="${SECRET_NAME:-vipertrade-secrets}"
APPLY="${APPLY:-false}"  # Set to true to actually apply (dry-run by default)

vt_require_cmd kubectl

vt_print_header "ViperTrade - Update Secrets from .env"

# Check env file exists
if [[ ! -f "$ENV_FILE" ]]; then
  vt_fail "Env file not found: $ENV_FILE"
  vt_info "Create it first: cp compose/.env.example compose/.env && edit compose/.env"
  exit 1
fi

vt_ok "Env file found: $ENV_FILE"

# Show what will be updated
vt_step "Secret name: $SECRET_NAME"
vt_step "Namespace: $KIND_NAMESPACE"
vt_step "Context: $KIND_CONTEXT"

# Count variables
var_count=$(grep -v '^#' "$ENV_FILE" | grep -c '=' || true)
vt_info "Variables in $ENV_FILE: $var_count"

# Dry-run by default
if [[ "$APPLY" != "true" ]]; then
  vt_warn "DRY-RUN mode: no changes will be applied"
  vt_info "Set APPLY=true to update the secret"
  echo ""
  vt_info "Preview of generated Secret:"
  kubectl --context "$KIND_CONTEXT" -n "$KIND_NAMESPACE" \
    create secret generic "$SECRET_NAME" \
    --dry-run=client \
    --from-env-file="$ENV_FILE" \
    -o yaml | head -50
echo ""
vt_info "To apply: APPLY=true $0"
exit 0
fi

# Apply the secret
vt_step "Updating Secret in cluster..."
if kubectl --context "$KIND_CONTEXT" -n "$KIND_NAMESPACE" \
  create secret generic "$SECRET_NAME" \
  --dry-run=client \
  --from-env-file="$ENV_FILE" \
  -o yaml | kubectl --context "$KIND_CONTEXT" -n "$KIND_NAMESPACE" apply -f -; then
  vt_ok "Secret updated successfully"
else
  vt_fail "Failed to update secret"
  exit 1
fi

# Verify
vt_step "Verifying secret"
kubectl --context "$KIND_CONTEXT" -n "$KIND_NAMESPACE" get secret "$SECRET_NAME" -o yaml | grep -E 'BYBIT|POSTGRES|NEXTAUTH|OPERATOR' | head -10 || true

echo ""
vt_info "Secrets updated. Restart deployments to pick up changes:"
vt_info "  kubectl --context $KIND_CONTEXT -n $KIND_NAMESPACE rollout restart deployment/api"
vt_info "  kubectl --context $KIND_CONTEXT -n $KIND_NAMESPACE rollout restart deployment/executor"
vt_info "  kubectl --context $KIND_CONTEXT -n $KIND_NAMESPACE rollout restart deployment/strategy"
vt_info "  kubectl --context $KIND_CONTEXT -n $KIND_NAMESPACE rollout restart deployment/monitor"
vt_info ""
vt_info "Or restart all at once:"
vt_info "  for dep in api executor strategy monitor; do"
vt_info "    kubectl --context $KIND_CONTEXT -n $KIND_NAMESPACE rollout restart deployment/\$dep;"
vt_info "  done"
