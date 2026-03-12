#!/usr/bin/env bash
set -euo pipefail

# Replay real Bybit mainnet candles into local Redis channel viper:market_data
# for strategy/executor simulation without placing real orders.
#
# Usage:
#   ./scripts/replay-mainnet-candles.sh DOGEUSDT 1 240 0.2
#   ./scripts/replay-mainnet-candles.sh XRPUSDT 5 500 0
#
# Args:
#   1: symbol          (default: DOGEUSDT)
#   2: interval        (default: 1)
#   3: candles_limit   (default: 240, max 1000)
#   4: sleep_seconds   (default: 0.1)

SYMBOL="${1:-DOGEUSDT}"
INTERVAL="${2:-1}"
LIMIT="${3:-240}"
SLEEP_SECONDS="${4:-0.1}"

if ! [[ "$LIMIT" =~ ^[0-9]+$ ]] || (( LIMIT < 1 )) || (( LIMIT > 1000 )); then
  echo "ERROR: candles_limit must be an integer between 1 and 1000"
  exit 1
fi

ART_DIR="docs/operations/artifacts/mainnet-candles"
mkdir -p "$ART_DIR"
TS_UTC="$(date -u +%Y%m%dT%H%M%SZ)"
RAW_JSON="$ART_DIR/${SYMBOL}_${INTERVAL}_${LIMIT}_${TS_UTC}.raw.json"
CSV_FILE="$ART_DIR/${SYMBOL}_${INTERVAL}_${LIMIT}_${TS_UTC}.csv"

BYBIT_URL="https://api.bybit.com/v5/market/kline?category=linear&symbol=${SYMBOL}&interval=${INTERVAL}&limit=${LIMIT}"

echo "[1/5] Fetching mainnet candles: $BYBIT_URL"
curl -fsSL "$BYBIT_URL" -o "$RAW_JSON"

echo "[2/5] Validating and preparing chronological dataset"
python3 - "$RAW_JSON" "$CSV_FILE" <<'PY'
import csv
import datetime as dt
import json
import sys

raw_path, csv_path = sys.argv[1], sys.argv[2]
with open(raw_path, "r", encoding="utf-8") as f:
    data = json.load(f)

ret_code = data.get("retCode")
if ret_code != 0:
    raise SystemExit(f"Bybit API error retCode={ret_code} retMsg={data.get('retMsg')}")

rows = data.get("result", {}).get("list", [])
if not rows:
    raise SystemExit("No candles returned by Bybit")

# Bybit returns newest first; replay should be oldest -> newest.
rows = list(reversed(rows))

with open(csv_path, "w", newline="", encoding="utf-8") as f:
    w = csv.writer(f)
    w.writerow([
        "start_time_utc",
        "open",
        "high",
        "low",
        "close",
        "volume",
        "turnover",
    ])
    for row in rows:
        # row format: [startTime, openPrice, highPrice, lowPrice, closePrice, volume, turnover]
        ts_ms = int(row[0])
        ts_iso = dt.datetime.fromtimestamp(ts_ms / 1000, dt.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")
        w.writerow([ts_iso, row[1], row[2], row[3], row[4], row[5], row[6]])

print(f"candles={len(rows)}")
PY

if ! podman ps --format '{{.Names}}' | rg -qx 'vipertrade-redis'; then
  echo "ERROR: vipertrade-redis container is not running"
  exit 1
fi

echo "[3/5] Replaying candles to Redis channel viper:market_data"
python3 - "$CSV_FILE" "$SYMBOL" "$SLEEP_SECONDS" <<'PY'
import csv
import json
import subprocess
import sys
import time
import uuid

csv_path, symbol, sleep_s = sys.argv[1], sys.argv[2], float(sys.argv[3])
published = 0

with open(csv_path, "r", encoding="utf-8") as f:
    reader = csv.DictReader(f)
    for r in reader:
        close = float(r["close"])
        high = float(r["high"])
        low = float(r["low"])
        volume = float(r["volume"])
        atr_14 = max((high - low), close * 0.002)

        event = {
            "schema_version": "1.0",
            "event_id": str(uuid.uuid4()),
            "timestamp": r["start_time_utc"],
            "signal": {
                "symbol": symbol,
                "current_price": close,
                "atr_14": atr_14,
                "volume_24h": volume,
                "funding_rate": 0.0,
                "trend_score": 0.5,
                "spread_pct": 0.001,
            },
        }

        payload = json.dumps(event, separators=(",", ":"))
        subprocess.run(
            [
                "podman",
                "exec",
                "vipertrade-redis",
                "redis-cli",
                "PUBLISH",
                "viper:market_data",
                payload,
            ],
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        published += 1
        if sleep_s > 0:
            time.sleep(sleep_s)

print(f"published={published}")
PY

echo "[4/5] Snapshot after replay"
curl -fsSL "http://localhost:8080/api/v1/status" | sed 's/^/status: /'
curl -fsSL "http://localhost:8080/api/v1/trades?limit=5" | sed 's/^/trades: /'

cat <<MSG
[5/5] Done
- Raw candles: $RAW_JSON
- CSV candles: $CSV_FILE
- Replay channel: viper:market_data
- Symbol: $SYMBOL
- Interval: $INTERVAL
- Limit: $LIMIT
- Sleep seconds: $SLEEP_SECONDS

Tip:
- Keep EXECUTOR_ENABLE_LIVE_ORDERS=false for safe simulation mode.
MSG
