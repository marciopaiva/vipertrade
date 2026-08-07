#!/usr/bin/env bash
# Publica o pairs.yaml local como ConfigMap, sem rebuild de imagem.
#
# O pairs.yaml é gitignored (tuning privado), então ele não pode viver dentro de
# um manifesto versionado — este script gera o ConfigMap a partir do arquivo
# local, do mesmo jeito que update-secrets.sh faz com o .env.
#
# Antes, mudar um parâmetro exigia `make build` (~10 min) porque o arquivo era
# assado na imagem. E reiniciar só alguns pods deixava a config divergente entre
# serviços — foi assim que a API serviu TP-ARM errado por 28h.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../lib/common.sh"
vt_cd_root

NAMESPACE="${KUBE_NAMESPACE:-vipertrade}"
CM_NAME="${PAIRS_CONFIGMAP:-vipertrade-pairs}"
PAIRS_FILE="${PAIRS_FILE:-config/trading/pairs.yaml}"
# Serviços que leem STRATEGY_CONFIG. Reiniciados juntos de propósito: config
# divergente entre eles produz números errados sem nenhum erro aparente.
DEPLOYS=(strategy executor api ai-analyst)

vt_require_cmd kubectl

[[ -f "$PAIRS_FILE" ]] || vt_fail "arquivo não encontrado: $PAIRS_FILE"

vt_print_header "ViperTrade — publicar pairs.yaml (sem rebuild)"
vt_step "arquivo:   $PAIRS_FILE ($(wc -l < "$PAIRS_FILE") linhas)"
vt_step "configmap: $CM_NAME"
vt_step "namespace: $NAMESPACE"

kubectl create configmap "$CM_NAME" \
  --namespace "$NAMESPACE" \
  --from-file=pairs.yaml="$PAIRS_FILE" \
  --dry-run=client -o yaml | kubectl apply -f -

if [[ "${RESTART:-true}" == "true" ]]; then
  vt_step "reiniciando os serviços que leem a config"
  for d in "${DEPLOYS[@]}"; do
    kubectl -n "$NAMESPACE" rollout restart "deployment/$d" >/dev/null
  done
  for d in "${DEPLOYS[@]}"; do
    kubectl -n "$NAMESPACE" rollout status "deployment/$d" --timeout=300s >/dev/null && \
      vt_ok "$d pronto"
  done
fi

vt_ok "config publicada — nenhuma imagem foi reconstruída"
