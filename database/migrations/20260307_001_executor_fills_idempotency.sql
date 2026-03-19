-- Migration: executor fills persistence + strong idempotency
-- Date: 2026-03-07

CREATE TABLE IF NOT EXISTS bybit_fills (
    fill_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    bybit_execution_id TEXT NOT NULL UNIQUE,
    bybit_order_id TEXT NOT NULL,
    order_link_id TEXT,
    symbol TEXT NOT NULL,
    side TEXT,
    exec_qty NUMERIC NOT NULL CHECK (exec_qty > 0),
    exec_price NUMERIC,
    exec_fee NUMERIC NOT NULL DEFAULT 0,
    fee_currency TEXT,
    is_maker BOOLEAN,
    exec_time TIMESTAMPTZ,
    raw_data JSONB NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_bybit_fills_order_id ON bybit_fills(bybit_order_id);
CREATE INDEX IF NOT EXISTS idx_bybit_fills_symbol ON bybit_fills(symbol);
CREATE INDEX IF NOT EXISTS idx_bybit_fills_exec_time ON bybit_fills(exec_time);

CREATE UNIQUE INDEX IF NOT EXISTS uq_system_events_executor_source_event
    ON system_events (event_type, (data->>'source_event_id'))
    WHERE event_type = 'executor_event_processed'
      AND COALESCE(data->>'source_event_id', '') <> '';

COMMENT ON TABLE bybit_fills IS 'Detailed fills received from Bybit for audit and reconciliation';
