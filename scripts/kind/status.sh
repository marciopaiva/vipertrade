#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/../lib/common.sh"

KIND_CONTEXT="${KIND_CONTEXT:-kind-dev}"
KIND_NAMESPACE="${KIND_NAMESPACE:-vipertrade}"

vt_require_cmd kubectl

vt_print_header "ViperTrade Kind status"
vt_step "Pods"
kubectl --context "$KIND_CONTEXT" -n "$KIND_NAMESPACE" get pods -o wide

vt_step "Deployments"
kubectl --context "$KIND_CONTEXT" -n "$KIND_NAMESPACE" get deployments

vt_step "Services"
kubectl --context "$KIND_CONTEXT" -n "$KIND_NAMESPACE" get svc

vt_step "Events (last 10)"
kubectl --context "$KIND_CONTEXT" -n "$KIND_NAMESPACE" get events --sort-by=.metadata.creationTimestamp | tail -10

vt_info "Cluster info:"
kubectl --context "$KIND_CONTEXT" get nodes -o wide

