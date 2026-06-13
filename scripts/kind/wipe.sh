#!/usr/bin/env bash
set -euo pipefail

# Wipe the paper trading data from the Kind cluster's postgres, then restart the
# stateful services so they come back on a clean slate (no stale in-memory open
# positions). Structural/config tables (schema_migrations, risk_profiles,
# trading_pairs_config) are kept — config lives in pairs.yaml (git), not the DB.

KUBE_NAMESPACE="${KUBE_NAMESPACE:-vipertrade}"
POSTGRES_USER="${POSTGRES_USER:-viper}"
POSTGRES_DB="${POSTGRES_DB:-vipertrade}"

# Runtime data accumulated by paper trading — everything that should reset to zero.
DATA_TABLES=(
  trades
  position_snapshots
  strategy_decision_audit
  tupa_audit_logs
  exchange_signal_snapshots
  system_events
  bybit_fills
  daily_metrics
  circuit_breaker_events
  profile_history
)

# Stateful services to restart after the wipe (clear in-memory state).
RESTART_DEPLOYS=(strategy executor market-data monitor analytics ai-analyst)

psql() {
  kubectl exec -n "$KUBE_NAMESPACE" deploy/postgres -- \
    env PGPASSWORD="${PGPASSWORD:-}" psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" "$@"
}

counts() {
  local sel=""
  for t in "${DATA_TABLES[@]}"; do
    sel+="(SELECT COUNT(*) FROM $t),"
  done
  psql -At -F '|' -c "SELECT ${sel%,};"
}

echo "== ViperTrade — wipe paper data (Kind: $KUBE_NAMESPACE) =="
echo "Tables: ${DATA_TABLES[*]}"
echo ""
echo "Before:"
paste -d' ' <(printf '%s\n' "${DATA_TABLES[@]}") <(counts | tr '|' '\n') | awk '{printf "  %-26s %s\n", $1, $2}'
echo ""

if [[ "${1:-}" != "--yes" && "${CONFIRM:-}" != "yes" ]]; then
  read -r -p "Wipe ALL paper data and restart services? [y/N] " ans
  [[ "$ans" =~ ^[Yy]$ ]] || { echo "Aborted."; exit 0; }
fi

# Single transaction; RESTART IDENTITY resets sequences; CASCADE handles FKs.
JOINED=$(IFS=,; echo "${DATA_TABLES[*]}")
psql -c "TRUNCATE TABLE $JOINED RESTART IDENTITY CASCADE;"

echo ""
echo "After:"
paste -d' ' <(printf '%s\n' "${DATA_TABLES[@]}") <(counts | tr '|' '\n') | awk '{printf "  %-26s %s\n", $1, $2}'
echo ""

echo "Restarting services for a clean slate..."
for d in "${RESTART_DEPLOYS[@]}"; do
  kubectl rollout restart deployment "$d" -n "$KUBE_NAMESPACE" >/dev/null 2>&1 && echo "  restarted $d"
done
for d in "${RESTART_DEPLOYS[@]}"; do
  kubectl rollout status deployment "$d" -n "$KUBE_NAMESPACE" --timeout=120s >/dev/null 2>&1 \
    && echo "  ready $d" || echo "  WARN $d not ready in time"
done

echo ""
echo "✓ Paper data wiped. Fresh start — config is the baked pairs.yaml (git)."
