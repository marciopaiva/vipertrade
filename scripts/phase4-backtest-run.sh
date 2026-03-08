#!/usr/bin/env bash
set -euo pipefail

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

WINDOW_START=""
WINDOW_END=""
SEED=42
PROFILE="MEDIUM"

usage() {
  cat <<USAGE
Usage:
  ./scripts/phase4-backtest-run.sh \
    --window-start <UTC ISO-8601> \
    --window-end   <UTC ISO-8601> \
    [--seed <int>] \
    [--profile LOW|MEDIUM|HIGH]
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --window-start)
      WINDOW_START="${2:-}"
      shift 2
      ;;
    --window-end)
      WINDOW_END="${2:-}"
      shift 2
      ;;
    --seed)
      SEED="${2:-}"
      shift 2
      ;;
    --profile)
      PROFILE="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage
      exit 2
      ;;
  esac
done

if [[ -z "$WINDOW_START" || -z "$WINDOW_END" ]]; then
  echo "ERROR: --window-start and --window-end are required" >&2
  usage
  exit 2
fi

if ! [[ "$SEED" =~ ^[0-9]+$ ]]; then
  echo "ERROR: --seed must be an integer" >&2
  exit 2
fi

case "$PROFILE" in
  LOW|MEDIUM|HIGH) ;;
  *)
    echo "ERROR: --profile must be LOW, MEDIUM, or HIGH" >&2
    exit 2
    ;;
esac

cd "$ROOT_DIR"

PIPELINE_FILE="config/strategies/viper_smart_copy.tp"
PAIRS_FILE="config/trading/pairs.yaml"
PROFILES_FILE="config/system/profiles.yaml"

for required in "$PIPELINE_FILE" "$PAIRS_FILE" "$PROFILES_FILE"; do
  if [[ ! -f "$required" ]]; then
    echo -e "${RED}ERROR: required input file missing: $required${NC}" >&2
    exit 1
  fi
done

if ! GIT_SHA="$(git rev-parse --short HEAD 2>/dev/null)"; then
  echo -e "${RED}ERROR: unable to resolve git SHA${NC}" >&2
  exit 1
fi

PIPELINE_HASH="$(sha256sum "$PIPELINE_FILE" | awk '{print $1}')"
PAIRS_HASH="$(sha256sum "$PAIRS_FILE" | awk '{print $1}')"
PROFILES_HASH="$(sha256sum "$PROFILES_FILE" | awk '{print $1}')"

RUN_KEY_RAW="${WINDOW_START}|${WINDOW_END}|${SEED}|${PROFILE}|${GIT_SHA}"
RUN_ID="$(printf "%s" "$RUN_KEY_RAW" | sha256sum | awk '{print substr($1,1,16)}')"

TS_UTC="$(date -u +%Y%m%dT%H%M%SZ)"
DATE_UTC="$(date -u +%Y-%m-%d)"
CREATED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

ARTIFACT_DIR="docs/operations/artifacts/backtest"
JSON_FILE="$ARTIFACT_DIR/backtest_${TS_UTC}_seed${SEED}.json"
MD_FILE="docs/operations/PHASE4_BACKTEST_RUN_${DATE_UTC}.md"

mkdir -p "$ARTIFACT_DIR"

BACKTEST_HTTP="$(curl -s -o /tmp/viper_phase4_backtest_run_health.out -w "%{http_code}" http://localhost:8085/health || true)"
SERVICE_AVAILABLE=false
if [[ "$BACKTEST_HTTP" == "200" ]]; then
  SERVICE_AVAILABLE=true
fi

STATUS="baseline_collected"
if [[ "$SERVICE_AVAILABLE" != "true" ]]; then
  STATUS="baseline_partial"
fi

cat > "$JSON_FILE" <<JSON
{
  "schema_version": "v1",
  "run_id": "$RUN_ID",
  "created_at_utc": "$CREATED_AT",
  "git_sha": "$GIT_SHA",
  "window": {
    "start_utc": "$WINDOW_START",
    "end_utc": "$WINDOW_END"
  },
  "seed": $SEED,
  "profile": "$PROFILE",
  "input_hashes": {
    "pipeline_tp": "$PIPELINE_HASH",
    "pairs_yaml": "$PAIRS_HASH",
    "profiles_yaml": "$PROFILES_HASH"
  },
  "checks": {
    "backtest_health_http": $BACKTEST_HTTP,
    "service_available": $SERVICE_AVAILABLE
  },
  "metrics": {
    "total_trades": null,
    "win_rate": null,
    "total_pnl": null,
    "max_drawdown": null
  },
  "status": "$STATUS"
}
JSON

cat > "$MD_FILE" <<MD
# Phase 4 Backtest Run - ${DATE_UTC}

## Run Summary

- run_id: ${RUN_ID}
- created_at_utc: ${CREATED_AT}
- git_sha: ${GIT_SHA}
- window_start: ${WINDOW_START}
- window_end: ${WINDOW_END}
- seed: ${SEED}
- profile: ${PROFILE}
- backtest_health_http: ${BACKTEST_HTTP}
- service_available: ${SERVICE_AVAILABLE}
- status: ${STATUS}

## Artifacts

- JSON: ${JSON_FILE}

## Notes

- Metrics are placeholders until engine output is wired into this run contract.
MD

echo -e "${GREEN}SUCCESS: Phase 4 deterministic run artifact generated${NC}"
echo "JSON: $JSON_FILE"
echo "Markdown: $MD_FILE"

if [[ "$SERVICE_AVAILABLE" != "true" ]]; then
  echo -e "${YELLOW}WARN: backtest service is unavailable (HTTP ${BACKTEST_HTTP})${NC}"
fi
