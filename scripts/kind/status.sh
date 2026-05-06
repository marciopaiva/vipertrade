#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../lib/common.sh"

KIND_CONTEXT="${KIND_CONTEXT:-kind-dev}"
KIND_NAMESPACE="${KIND_NAMESPACE:-vipertrade}"

vt_require_cmd kubectl

vt_print_header "ViperTrade Kind status"
echo ""
echo -e "${VT_YELLOW}→ Pods${VT_NC}"
kubectl --context "$KIND_CONTEXT" -n "$KIND_NAMESPACE" get pods -o wide

echo ""
echo -e "${VT_YELLOW}→ Deployments${VT_NC}"
kubectl --context "$KIND_CONTEXT" -n "$KIND_NAMESPACE" get deployments

echo ""
echo -e "${VT_YELLOW}→ Services${VT_NC}"
kubectl --context "$KIND_CONTEXT" -n "$KIND_NAMESPACE" get svc

echo ""
echo -e "${VT_YELLOW}→ Pod Images & Phases${VT_NC}"
kubectl --context "$KIND_CONTEXT" -n "$KIND_NAMESPACE" get pods -o jsonpath='{range .items[*]}{.metadata.name}{"\t"}{.status.containerStatuses[0].image}{"\t"}{.status.phase}{"\n"}{end}'

echo ""
echo -e "${VT_YELLOW}→ Recent Events (last 10)${VT_NC}"
kubectl --context "$KIND_CONTEXT" -n "$KIND_NAMESPACE" get events --sort-by=.metadata.creationTimestamp | tail -10

echo ""
vt_info "Cluster info:"
kubectl --context "$KIND_CONTEXT" get nodes -o wide

