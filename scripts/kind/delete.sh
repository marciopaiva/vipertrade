#!/usr/bin/env bash
set -uo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/../lib/common.sh"

KIND_CONTEXT="${KIND_CONTEXT:-kind-dev}"

vt_require_cmd kubectl

vt_print_header "Delete ViperTrade from Kind"
kubectl --context "$KIND_CONTEXT" delete -k k8s/kind 2>&1 | grep -v "not found" || true

