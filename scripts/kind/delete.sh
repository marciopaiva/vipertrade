#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../lib/common.sh"

vt_cd_root

KIND_CONTEXT="${KIND_CONTEXT:-kind-dev}"

vt_require_cmd kubectl

vt_print_header "Delete ViperTrade from Kind"
kubectl --context "$KIND_CONTEXT" delete -k k8s/kind

