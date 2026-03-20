#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
. "$SCRIPT_DIR/lib/common.sh"
. "$SCRIPT_DIR/container-runtime.sh"

SYMBOL="${1:-DOGEUSDT}"
ACTION="${2:-ENTER_LONG}"
QTY="${3:-10}"

show_help() {
  vt_print_header "ViperTrade - Publish Test Decision"
  echo ""
  echo "Usage:"
  echo "  ./scripts/publish-test-decision.sh [SYMBOL] [ACTION] [QTY]"
  echo ""
  echo "Examples:"
  echo "  ./scripts/publish-test-decision.sh"
  echo "  ./scripts/publish-test-decision.sh DOGEUSDT ENTER_LONG 10"
  echo "  ./scripts/publish-test-decision.sh XRPUSDT CLOSE_SHORT 5"
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" || "${1:-}" == "help" ]]; then
  show_help
  exit 0
fi

TS="$(date -Iseconds)"
SRC_ID="manual-src-$(date +%s)"
EVT_ID="manual-decision-$(date +%s)-$RANDOM"

PAYLOAD=$(cat <<JSON
{
  "schema_version":"1.0",
  "event_id":"${EVT_ID}",
  "source_event_id":"${SRC_ID}",
  "timestamp":"${TS}",
  "decision":{
    "action":"${ACTION}",
    "symbol":"${SYMBOL}",
    "quantity":${QTY},
    "leverage":2.0,
    "entry_price":1.0,
    "stop_loss":0.98,
    "take_profit":1.04,
    "reason":"manual_test",
    "smart_copy_compatible":true
  }
}
JSON
)

container_exec vipertrade-redis redis-cli PUBLISH viper:decisions "${PAYLOAD}"
vt_ok "Published test decision event_id=${EVT_ID} action=${ACTION} symbol=${SYMBOL}"
