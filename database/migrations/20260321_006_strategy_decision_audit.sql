-- Migration: persist structured strategy decisions for diagnostics
-- Date: 2026-03-21

CREATE TABLE IF NOT EXISTS strategy_decision_audit (
    audit_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    source_event_id TEXT NOT NULL,
    decision_event_id TEXT NOT NULL UNIQUE,
    schema_version TEXT NOT NULL,
    symbol TEXT NOT NULL,
    action TEXT NOT NULL,
    reason TEXT NOT NULL,
    smart_copy_compatible BOOLEAN NOT NULL DEFAULT FALSE,
    decision_hash TEXT NOT NULL,
    executor_status TEXT NOT NULL,
    bybit_order_id TEXT,
    error TEXT,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_strategy_decision_audit_created_at
    ON strategy_decision_audit(created_at);
CREATE INDEX IF NOT EXISTS idx_strategy_decision_audit_symbol_created_at
    ON strategy_decision_audit(symbol, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_strategy_decision_audit_action_created_at
    ON strategy_decision_audit(action, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_strategy_decision_audit_status_created_at
    ON strategy_decision_audit(executor_status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_strategy_decision_audit_reason
    ON strategy_decision_audit(reason);
CREATE INDEX IF NOT EXISTS idx_strategy_decision_audit_payload_gin
    ON strategy_decision_audit USING GIN(payload);

COMMENT ON TABLE strategy_decision_audit IS 'Structured strategy decisions with executor outcome for operational diagnostics';
