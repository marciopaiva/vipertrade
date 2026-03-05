-- VIPERTRADE SCHEMA v0.8.1
-- Extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Tabela 1: trades (Histórico imutável)
CREATE TABLE trades (
    trade_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    bybit_order_id TEXT UNIQUE,
    symbol TEXT NOT NULL,
    side TEXT NOT NULL CHECK (side IN ('Long', 'Short')),
    quantity NUMERIC NOT NULL,
    entry_price NUMERIC NOT NULL,
    exit_price NUMERIC,
    leverage NUMERIC NOT NULL DEFAULT 2,
    pnl NUMERIC,
    pnl_pct NUMERIC,
    fees NUMERIC DEFAULT 0,
    funding_paid NUMERIC DEFAULT 0,
    opened_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    closed_at TIMESTAMPTZ,
    status TEXT NOT NULL DEFAULT 'open',
    close_reason TEXT,
    pipeline_version TEXT,
    decision_hash TEXT,
    paper_trade BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Tabela 2: position_snapshots (Reconciliação)
CREATE TABLE position_snapshots (
    snapshot_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    symbol TEXT NOT NULL,
    bybit_data JSONB NOT NULL,
    local_calculation JSONB NOT NULL,
    divergence NUMERIC,
    divergence_pct NUMERIC,
    reconciled BOOLEAN DEFAULT FALSE,
    snapshot_at TIMESTAMPTZ DEFAULT NOW(),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Tabela 3: system_events (Audit trail)
CREATE TABLE system_events (
    event_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_type TEXT NOT NULL,
    severity TEXT NOT NULL DEFAULT 'info',
    category TEXT,
    data JSONB NOT NULL,
    symbol TEXT,
    trade_id UUID REFERENCES trades(trade_id),
    timestamp TIMESTAMPTZ DEFAULT NOW(),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Tabela 4: daily_metrics (Agregados)
CREATE TABLE daily_metrics (
    date DATE PRIMARY KEY,
    total_trades INTEGER DEFAULT 0,
    winning_trades INTEGER DEFAULT 0,
    losing_trades INTEGER DEFAULT 0,
    win_rate NUMERIC DEFAULT 0,
    total_pnl NUMERIC DEFAULT 0,
    total_pnl_pct NUMERIC DEFAULT 0,
    total_fees NUMERIC DEFAULT 0,
    max_drawdown NUMERIC DEFAULT 0,
    by_pair JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Tabela 5: profile_history (Mudanças de perfil)
CREATE TABLE profile_history (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    profile_code TEXT NOT NULL,
    changed_at TIMESTAMPTZ DEFAULT NOW(),
    reason TEXT,
    previous_profile TEXT,
    user_id TEXT
);

-- Tabela 6: schema_migrations (Versionamento)
CREATE TABLE schema_migrations (
    version TEXT PRIMARY KEY,
    applied_at TIMESTAMPTZ DEFAULT NOW()
);

-- Índices de performance
CREATE INDEX idx_trades_status ON trades(status);
CREATE INDEX idx_trades_symbol ON trades(symbol);
CREATE INDEX idx_trades_opened_at ON trades(opened_at);
CREATE INDEX idx_trades_pnl ON trades(pnl);
CREATE INDEX idx_snapshots_reconciled ON position_snapshots(reconciled);
CREATE INDEX idx_events_severity ON system_events(severity);
CREATE INDEX idx_events_timestamp ON system_events(timestamp);

-- Seed initial migration version
INSERT INTO schema_migrations (version) VALUES ('0.8.1');
