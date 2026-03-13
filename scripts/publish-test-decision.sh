#!/bin/bash
set -euo pipefail
. "$(dirname "$0")/container-runtime.sh"

SYMBOL="${1:-DOGEUSDT}"
ACTION="${2:-ENTER_LONG}"
QTY="${3:-10}"

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
echo "Published test decision event_id=${EVT_ID} action=${ACTION} symbol=${SYMBOL}"
