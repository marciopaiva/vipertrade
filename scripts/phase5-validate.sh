#!/usr/bin/env bash
set -euo pipefail

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
DATE_UTC="$(date -u +%Y-%m-%d)"
TS_UTC="$(date -u +%Y%m%dT%H%M%SZ)"
CREATED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
WINDOW_HOURS="${WINDOW_HOURS:-24}"
SMART_COPY_MIN_IN_BAND_RATIO="${SMART_COPY_MIN_IN_BAND_RATIO:-0.95}"

ARTIFACT_DIR="$ROOT_DIR/docs/operations/artifacts/phase5"
JSON_FILE="$ARTIFACT_DIR/phase5_baseline_${TS_UTC}.json"
MD_FILE="$ROOT_DIR/docs/operations/PHASE5_BASELINE_${DATE_UTC}.md"

mkdir -p "$ARTIFACT_DIR"
cd "$ROOT_DIR"

echo -e "${GREEN}ViperTrade - Phase 5 Baseline Validation${NC}"
echo "================================================"

ISSUES=0

if ./scripts/health-check.sh >/tmp/viper_phase5_health.log 2>&1; then
  HEALTH_OK=true
  echo -e "${GREEN}OK: health-check passed${NC}"
else
  HEALTH_OK=false
  echo -e "${RED}ERROR: health-check failed${NC}"
  tail -n 80 /tmp/viper_phase5_health.log || true
  ISSUES=$((ISSUES + 1))
fi

if ./scripts/check-api-metrics-consistency.sh >/tmp/viper_phase5_perf.log 2>&1; then
  PERF_OK=true
  echo -e "${GREEN}OK: API performance consistency passed${NC}"
else
  PERF_OK=false
  echo -e "${RED}ERROR: API performance consistency failed${NC}"
  tail -n 80 /tmp/viper_phase5_perf.log || true
  ISSUES=$((ISSUES + 1))
fi

CONFIG_JSON="$(python3 - <<'PY'
import json
from pathlib import Path
import yaml

pairs = yaml.safe_load(Path("config/trading/pairs.yaml").read_text(encoding="utf-8"))
profiles_cfg = yaml.safe_load(Path("config/system/profiles.yaml").read_text(encoding="utf-8"))

profiles = [k for k, v in profiles_cfg.items() if isinstance(v, dict)]
enabled_pairs = [
    k for k, v in pairs.items()
    if k != "global" and isinstance(v, dict) and v.get("enabled", False)
]

global_sc = pairs.get("global", {}).get("smart_copy", {}) if isinstance(pairs.get("global"), dict) else {}
sc_min = float(global_sc.get("min_position_usdt", 0))
sc_max = float(global_sc.get("max_position_usdt", 0))
smart_copy_band_valid = sc_min > 0 and sc_max > 0 and sc_min < sc_max

required_trailing_keys = {
    "activate_after_profit_pct",
    "initial_trail_pct",
    "ratchet_levels",
    "move_to_break_even_at",
}

pair_missing = {}
pair_malformed = {}
for symbol in enabled_pairs:
    cfg = pairs.get(symbol, {})
    by_profile = cfg.get("trailing_stop", {}).get("by_profile", {})
    missing_profiles = [p for p in profiles if p not in by_profile]
    if missing_profiles:
        pair_missing[symbol] = missing_profiles
        continue

    malformed = []
    for p in profiles:
        p_cfg = by_profile.get(p, {})
        missing_keys = sorted(required_trailing_keys - set(p_cfg.keys()))
        if missing_keys:
            malformed.append({"profile": p, "missing_keys": missing_keys})
            continue
        ratchets = p_cfg.get("ratchet_levels", [])
        if not isinstance(ratchets, list):
            malformed.append({"profile": p, "error": "ratchet_levels_not_list"})
            continue
        for idx, level in enumerate(ratchets):
            if not isinstance(level, dict) or "at_profit_pct" not in level or "trail_pct" not in level:
                malformed.append({"profile": p, "error": "invalid_ratchet_level", "index": idx})
                break
    if malformed:
        pair_malformed[symbol] = malformed

trailing_profiles_complete = not pair_missing and not pair_malformed

print(json.dumps({
    "profiles": profiles,
    "enabled_pairs": enabled_pairs,
    "smart_copy_min_position_usdt": sc_min,
    "smart_copy_max_position_usdt": sc_max,
    "smart_copy_band_valid": smart_copy_band_valid,
    "trailing_profiles_complete": trailing_profiles_complete,
    "missing_profiles_by_pair": pair_missing,
    "malformed_profiles_by_pair": pair_malformed,
}))
PY
)"

SMART_COPY_MIN_USDT="$(python3 - <<'PY' "$CONFIG_JSON"
import json,sys
print(json.loads(sys.argv[1])["smart_copy_min_position_usdt"])
PY
)"
SMART_COPY_MAX_USDT="$(python3 - <<'PY' "$CONFIG_JSON"
import json,sys
print(json.loads(sys.argv[1])["smart_copy_max_position_usdt"])
PY
)"
SMART_COPY_BAND_VALID="$(python3 - <<'PY' "$CONFIG_JSON"
import json,sys
print("true" if json.loads(sys.argv[1])["smart_copy_band_valid"] else "false")
PY
)"
TRAILING_PROFILES_COMPLETE="$(python3 - <<'PY' "$CONFIG_JSON"
import json,sys
print("true" if json.loads(sys.argv[1])["trailing_profiles_complete"] else "false")
PY
)"
MISSING_PROFILES_JSON="$(python3 - <<'PY' "$CONFIG_JSON"
import json,sys
print(json.dumps(json.loads(sys.argv[1])["missing_profiles_by_pair"]))
PY
)"
MALFORMED_PROFILES_JSON="$(python3 - <<'PY' "$CONFIG_JSON"
import json,sys
print(json.dumps(json.loads(sys.argv[1])["malformed_profiles_by_pair"]))
PY
)"

if [[ "$SMART_COPY_BAND_VALID" == "true" ]]; then
  echo -e "${GREEN}OK: smart copy notional band is valid (${SMART_COPY_MIN_USDT}..${SMART_COPY_MAX_USDT})${NC}"
else
  echo -e "${RED}ERROR: invalid smart copy notional band (${SMART_COPY_MIN_USDT}..${SMART_COPY_MAX_USDT})${NC}"
  ISSUES=$((ISSUES + 1))
fi

if [[ "$TRAILING_PROFILES_COMPLETE" == "true" ]]; then
  echo -e "${GREEN}OK: trailing profile coverage complete for enabled pairs${NC}"
else
  echo -e "${RED}ERROR: trailing profile coverage has missing/malformed entries${NC}"
  ISSUES=$((ISSUES + 1))
fi

DB_SIGNALS_OK=true
DB_COUNTS=$(podman exec -i vipertrade-postgres psql -U "${POSTGRES_USER:-viper}" -d "${POSTGRES_DB:-vipertrade}" -At -F '|' -c \
  "WITH window_trades AS (
     SELECT quantity * entry_price AS notional
     FROM trades
     WHERE status='closed'
       AND closed_at >= NOW() - INTERVAL '${WINDOW_HOURS} hours'
       AND smart_copy_compatible = true
   )
   SELECT
     (SELECT COUNT(*)::bigint FROM window_trades),
     (SELECT COUNT(*)::bigint FROM window_trades WHERE notional >= ${SMART_COPY_MIN_USDT} AND notional <= ${SMART_COPY_MAX_USDT});" \
  2>/tmp/viper_phase5_db.log) || DB_SIGNALS_OK=false

if [[ "$DB_SIGNALS_OK" == "true" ]]; then
  SMART_COPY_TRADES_TOTAL="$(echo "$DB_COUNTS" | awk -F'|' '{print $1}')"
  SMART_COPY_TRADES_IN_BAND="$(echo "$DB_COUNTS" | awk -F'|' '{print $2}')"
else
  SMART_COPY_TRADES_TOTAL=-1
  SMART_COPY_TRADES_IN_BAND=-1
  echo -e "${RED}ERROR: DB signal query failed${NC}"
  tail -n 80 /tmp/viper_phase5_db.log 2>/dev/null || true
  ISSUES=$((ISSUES + 1))
fi

IN_BAND_RATIO="$(python3 - <<'PY' "$SMART_COPY_TRADES_TOTAL" "$SMART_COPY_TRADES_IN_BAND"
import sys
total=int(sys.argv[1]); in_band=int(sys.argv[2])
if total <= 0:
    print("1.0")
else:
    print(f"{in_band/total:.6f}")
PY
)"

IN_BAND_OK="$(python3 - <<'PY' "$IN_BAND_RATIO" "$SMART_COPY_MIN_IN_BAND_RATIO"
import sys
ratio=float(sys.argv[1]); threshold=float(sys.argv[2])
print("true" if ratio >= threshold else "false")
PY
)"

if [[ "$IN_BAND_OK" == "true" ]]; then
  echo -e "${GREEN}OK: smart copy in-band ratio=${IN_BAND_RATIO} (threshold=${SMART_COPY_MIN_IN_BAND_RATIO})${NC}"
else
  echo -e "${RED}ERROR: smart copy in-band ratio=${IN_BAND_RATIO} below threshold=${SMART_COPY_MIN_IN_BAND_RATIO}${NC}"
  ISSUES=$((ISSUES + 1))
fi

STATUS="passed"
if (( ISSUES > 0 )); then
  STATUS="failed"
fi

cat > "$JSON_FILE" <<JSON
{
  "schema_version": "v1",
  "created_at_utc": "$CREATED_AT",
  "status": "$STATUS",
  "window_hours": $WINDOW_HOURS,
  "thresholds": {
    "smart_copy_min_in_band_ratio": $SMART_COPY_MIN_IN_BAND_RATIO
  },
  "checks": {
    "health_check": $HEALTH_OK,
    "api_performance_consistency": $PERF_OK,
    "smart_copy_band_valid": $SMART_COPY_BAND_VALID,
    "trailing_profiles_complete": $TRAILING_PROFILES_COMPLETE,
    "smart_copy_in_band_ratio_ok": $IN_BAND_OK
  },
  "signals": {
    "smart_copy_min_position_usdt": $SMART_COPY_MIN_USDT,
    "smart_copy_max_position_usdt": $SMART_COPY_MAX_USDT,
    "smart_copy_trades_total_window": $SMART_COPY_TRADES_TOTAL,
    "smart_copy_trades_in_band_window": $SMART_COPY_TRADES_IN_BAND,
    "smart_copy_in_band_ratio": $IN_BAND_RATIO
  },
  "details": {
    "missing_profiles_by_pair": $MISSING_PROFILES_JSON,
    "malformed_profiles_by_pair": $MALFORMED_PROFILES_JSON
  },
  "issues": $ISSUES
}
JSON

cat > "$MD_FILE" <<MD
# Phase 5 Baseline Validation - ${DATE_UTC}

## Summary

- status: ${STATUS}
- issues: ${ISSUES}
- health_check: ${HEALTH_OK}
- api_performance_consistency: ${PERF_OK}
- smart_copy_band_valid: ${SMART_COPY_BAND_VALID}
- trailing_profiles_complete: ${TRAILING_PROFILES_COMPLETE}
- smart_copy_in_band_ratio_ok: ${IN_BAND_OK}
- smart_copy_band_usdt: ${SMART_COPY_MIN_USDT}..${SMART_COPY_MAX_USDT}
- smart_copy_trades_total_${WINDOW_HOURS}h: ${SMART_COPY_TRADES_TOTAL}
- smart_copy_trades_in_band_${WINDOW_HOURS}h: ${SMART_COPY_TRADES_IN_BAND}
- smart_copy_in_band_ratio_${WINDOW_HOURS}h: ${IN_BAND_RATIO}

## Artifact

- ${JSON_FILE#$ROOT_DIR/}
MD

echo "Evidence JSON: ${JSON_FILE#$ROOT_DIR/}"
echo "Evidence MD:   ${MD_FILE#$ROOT_DIR/}"

if (( ISSUES == 0 )); then
  echo -e "${GREEN}SUCCESS: Phase 5 baseline validation passed${NC}"
  exit 0
fi

echo -e "${YELLOW}WARN: Phase 5 baseline validation found ${ISSUES} issue(s)${NC}"
exit 1
