-- database/schema.sql
-- ViperTrade Database Schema v0.8.0-rc
-- Otimizado para Tupã audit logging e Smart Copy tracking

-- ═══════════════════════════════════════════════════════════
-- EXTENSIONS
-- ═══════════════════════════════════════════════════════════

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pg_stat_statements";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";  -- Para hashes seguros

-- ═══════════════════════════════════════════════════════════
-- TABLE 1: trades (Histórico imutável de operações)
-- ═══════════════════════════════════════════════════════════

CREATE TABLE trades (
    -- Identificação única
    trade_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    bybit_order_id TEXT UNIQUE,  -- ID da ordem na Bybit
    order_link_id TEXT UNIQUE,   -- Nosso ID para idempotência
    
    -- Símbolo e direção
    symbol TEXT NOT NULL CHECK (symbol IN ('DOGEUSDT', 'XRPUSDT', 'TRXUSDT', 'XLMUSDT')),
    side TEXT NOT NULL CHECK (side IN ('Long', 'Short')),
    
    -- Detalhes da posição
    quantity NUMERIC NOT NULL CHECK (quantity > 0),
    entry_price NUMERIC NOT NULL CHECK (entry_price > 0),
    exit_price NUMERIC,
    leverage NUMERIC NOT NULL DEFAULT 2 CHECK (leverage >= 1 AND leverage <= 3),
    
    -- P&L e custos
    pnl NUMERIC,
    pnl_pct NUMERIC,
    fees NUMERIC DEFAULT 0,
    funding_paid NUMERIC DEFAULT 0,
    slippage_pct NUMERIC DEFAULT 0,
    
    -- Timing
    opened_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    closed_at TIMESTAMPTZ,
    duration_seconds INTEGER GENERATED ALWAYS AS (
        EXTRACT(EPOCH FROM (closed_at - opened_at))::INTEGER
    ) STORED,
    
    -- Status e motivo de fechamento
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'closed', 'liquidated', 'cancelled', 'rejected')),
    close_reason TEXT CHECK (close_reason IN ('take_profit', 'stop_loss', 'trailing_stop', 'time_exit', 'manual', 'liquidation', 'circuit_breaker', 'error')),
    
    -- Tupã Engine Audit (CRÍTICO para compliance)
    pipeline_version TEXT NOT NULL DEFAULT '0.8.0-rc',
    decision_hash TEXT NOT NULL,  -- SHA-256 da decisão Tupã
    execution_hash TEXT,          -- SHA-256 da execução real
    constraints_satisfied BOOLEAN DEFAULT true,
    
    -- Smart Copy metadata
    smart_copy_compatible BOOLEAN DEFAULT true,
    copy_ratio NUMERIC DEFAULT 1.0,  -- Ratio aplicado para followers
    
    -- Trailing stop tracking
    trailing_stop_activated BOOLEAN DEFAULT false,
    trailing_stop_peak_price NUMERIC,
    trailing_stop_final_distance_pct NUMERIC,
    
    -- Perfil de risco usado
    trading_profile TEXT NOT NULL DEFAULT 'MEDIUM' CHECK (trading_profile IN ('CONSERVATIVE', 'MEDIUM', 'AGGRESSIVE')),
    
    -- Paper trading flag
    paper_trade BOOLEAN DEFAULT FALSE,
    
    -- Timestamps de auditoria
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Índices de performance
CREATE INDEX idx_trades_status ON trades(status);
CREATE INDEX idx_trades_symbol ON trades(symbol);
CREATE INDEX idx_trades_opened_at ON trades(opened_at);
CREATE INDEX idx_trades_closed_at ON trades(closed_at);
CREATE INDEX idx_trades_pnl ON trades(pnl);
CREATE INDEX idx_trades_decision_hash ON trades(decision_hash);
CREATE INDEX idx_trades_paper_trade ON trades(paper_trade);
CREATE INDEX idx_trades_profile ON trades(trading_profile);

-- ═══════════════════════════════════════════════════════════
-- TABLE 2: position_snapshots (Para reconciliação Bybit vs Local)
-- ═══════════════════════════════════════════════════════════

CREATE TABLE position_snapshots (
    snapshot_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    symbol TEXT NOT NULL CHECK (symbol IN ('DOGEUSDT', 'XRPUSDT', 'TRXUSDT', 'XLMUSDT')),
    
    -- Dados crus da Bybit (JSON flexível para evolução da API)
    bybit_data JSONB NOT NULL,
    
    -- Cálculo local do ViperTrade
    local_calculation JSONB NOT NULL,
    
    -- Tracking de divergência
    divergence NUMERIC,
    divergence_pct NUMERIC,
    reconciled BOOLEAN DEFAULT FALSE,
    reconciliation_notes TEXT,
    
    -- Timing
    snapshot_at TIMESTAMPTZ DEFAULT NOW(),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Índices
CREATE INDEX idx_snapshots_symbol ON position_snapshots(symbol);
CREATE INDEX idx_snapshots_reconciled ON position_snapshots(reconciled);
CREATE INDEX idx_snapshots_created_at ON position_snapshots(created_at);
CREATE INDEX idx_snapshots_divergence ON position_snapshots(divergence_pct) WHERE divergence_pct > 0.01;

-- ═══════════════════════════════════════════════════════════
-- TABLE 3: system_events (Audit trail de todos os eventos)
-- ═══════════════════════════════════════════════════════════

CREATE TABLE system_events (
    event_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    
    -- Classificação do evento
    event_type TEXT NOT NULL,
    severity TEXT NOT NULL DEFAULT 'info' CHECK (severity IN ('debug', 'info', 'warning', 'error', 'critical')),
    category TEXT CHECK (category IN ('trade', 'risk', 'system', 'notification', 'reconciliation', 'tupa', 'circuit_breaker')),
    
    -- Dados do evento (JSON estruturado)
    data JSONB NOT NULL,
    
    -- Contexto opcional
    symbol TEXT,
    trade_id UUID REFERENCES trades(trade_id) ON DELETE SET NULL,
    pipeline_version TEXT,
    decision_hash TEXT,
    
    -- Timing
    timestamp TIMESTAMPTZ DEFAULT NOW(),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Índices para queries de auditoria
CREATE INDEX idx_events_type ON system_events(event_type);
CREATE INDEX idx_events_severity ON system_events(severity);
CREATE INDEX idx_events_timestamp ON system_events(timestamp);
CREATE INDEX idx_events_symbol ON system_events(symbol);
CREATE INDEX idx_events_category ON system_events(category);
CREATE INDEX idx_events_critical ON system_events(severity, timestamp) WHERE severity IN ('error', 'critical');

-- Strong idempotency for executor processed events
CREATE UNIQUE INDEX uq_system_events_executor_source_event
    ON system_events (event_type, (data->>'source_event_id'))
    WHERE event_type = 'executor_event_processed'
      AND COALESCE(data->>'source_event_id', '') <> '';

-- ==========================================================
-- TABLE 3.1: bybit_fills (detailed fills for audit/reconcile)
-- ==========================================================

CREATE TABLE bybit_fills (
    fill_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    bybit_execution_id TEXT NOT NULL UNIQUE,
    bybit_order_id TEXT NOT NULL,
    order_link_id TEXT,
    symbol TEXT NOT NULL CHECK (symbol IN ('DOGEUSDT', 'XRPUSDT', 'TRXUSDT', 'XLMUSDT')),
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

CREATE INDEX idx_bybit_fills_order_id ON bybit_fills(bybit_order_id);
CREATE INDEX idx_bybit_fills_symbol ON bybit_fills(symbol);
CREATE INDEX idx_bybit_fills_exec_time ON bybit_fills(exec_time);

-- TABLE 4: daily_metrics (Agregados diários para dashboard)
-- ═══════════════════════════════════════════════════════════

CREATE TABLE daily_metrics (
    date DATE PRIMARY KEY,
    
    -- Trading Stats
    total_trades INTEGER DEFAULT 0,
    winning_trades INTEGER DEFAULT 0,
    losing_trades INTEGER DEFAULT 0,
    win_rate NUMERIC DEFAULT 0,
    
    -- P&L
    total_pnl NUMERIC DEFAULT 0,
    total_pnl_pct NUMERIC DEFAULT 0,
    total_fees NUMERIC DEFAULT 0,
    total_funding NUMERIC DEFAULT 0,
    avg_slippage_pct NUMERIC DEFAULT 0,
    
    -- Risk Metrics
    max_drawdown NUMERIC DEFAULT 0,
    max_position_size NUMERIC DEFAULT 0,
    circuit_breaker_triggers INTEGER DEFAULT 0,
    consecutive_losses INTEGER DEFAULT 0,
    
    -- Tupã Metrics
    pipeline_executions INTEGER DEFAULT 0,
    avg_execution_latency_ms NUMERIC DEFAULT 0,
    constraints_violations INTEGER DEFAULT 0,
    
    -- Smart Copy Metrics
    copy_success_rate NUMERIC DEFAULT 1.0,
    failed_copies INTEGER DEFAULT 0,
    
    -- By Pair (JSON para flexibilidade)
    by_pair JSONB DEFAULT '{}',
    
    -- Perfil breakdown
    by_profile JSONB DEFAULT '{}',
    
    -- Timing
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- ═══════════════════════════════════════════════════════════
-- TABLE 5: tupã_audit_logs (Logs estruturados do Tupã Engine)
-- ═══════════════════════════════════════════════════════════

CREATE TABLE tupa_audit_logs (
    log_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    
    -- Identificação da execução
    execution_id TEXT NOT NULL,  -- Hash único da execução Tupã
    pipeline_name TEXT NOT NULL,
    pipeline_version TEXT NOT NULL,
    
    -- Input/Output
    input_hash TEXT NOT NULL,  -- SHA-256 do input para reproducibilidade
    output_hash TEXT NOT NULL, -- SHA-256 do output
    decision_hash TEXT NOT NULL,
    
    -- Dados completos em JSON (para auditoria forense)
    input_data JSONB,
    output_data JSONB,
    constraints_results JSONB,
    
    -- Performance
    execution_time_ms INTEGER,
    memory_used_kb INTEGER,
    
    -- Ambiente de execução
    environment JSONB,  -- {arch, os, rust_version, tupa_version, etc.}
    
    -- Timing
    executed_at TIMESTAMPTZ DEFAULT NOW(),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Índices para auditoria
CREATE INDEX idx_audit_execution_id ON tupa_audit_logs(execution_id);
CREATE INDEX idx_audit_pipeline ON tupa_audit_logs(pipeline_name, pipeline_version);
CREATE INDEX idx_audit_timestamp ON tupa_audit_logs(executed_at);
CREATE INDEX idx_audit_decision ON tupa_audit_logs(decision_hash);

-- ═══════════════════════════════════════════════════════════
-- TABLE 6: profile_history (Tracking de mudanças de perfil)
-- ═══════════════════════════════════════════════════════════

CREATE TABLE profile_history (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    profile_code TEXT NOT NULL CHECK (profile_code IN ('CONSERVATIVE', 'MEDIUM', 'AGGRESSIVE')),
    changed_at TIMESTAMPTZ DEFAULT NOW(),
    reason TEXT,
    previous_profile TEXT,
    user_id TEXT,  -- Para multi-user no futuro
    metadata JSONB  -- Config snapshot no momento da mudança
);

CREATE INDEX idx_profile_history_changed_at ON profile_history(changed_at);

-- ═══════════════════════════════════════════════════════════
-- TABLE 7: circuit_breaker_events (Histórico de circuit breakers)
-- ═══════════════════════════════════════════════════════════

CREATE TABLE circuit_breaker_events (
    event_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    
    -- Tipo de breaker
    breaker_type TEXT NOT NULL CHECK (breaker_type IN ('daily_loss', 'consecutive_losses', 'volatility_spike', 'flash_crash', 'liquidation_cascade')),
    
    -- Gatilho
    trigger_value NUMERIC NOT NULL,
    threshold_value NUMERIC NOT NULL,
    
    -- Ação tomada
    action_taken TEXT NOT NULL,
    positions_affected INTEGER DEFAULT 0,
    
    -- Duração
    activated_at TIMESTAMPTZ DEFAULT NOW(),
    deactivated_at TIMESTAMPTZ,
    duration_seconds INTEGER GENERATED ALWAYS AS (
        EXTRACT(EPOCH FROM (deactivated_at - activated_at))::INTEGER
    ) STORED,
    
    -- Metadata
    metadata JSONB,
    
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_circuit_breaker_type ON circuit_breaker_events(breaker_type);
CREATE INDEX idx_circuit_breaker_activated ON circuit_breaker_events(activated_at);

-- ═══════════════════════════════════════════════════════════
-- TABLE 8: schema_migrations (Versionamento do schema)
-- ═══════════════════════════════════════════════════════════

CREATE TABLE schema_migrations (
    version TEXT PRIMARY KEY,
    applied_at TIMESTAMPTZ DEFAULT NOW(),
    checksum TEXT NOT NULL,  -- SHA-256 do script SQL
    description TEXT
);

-- ═══════════════════════════════════════════════════════════
-- FUNCTIONS & TRIGGERS
-- ═══════════════════════════════════════════════════════════

-- Auto-update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER update_trades_updated_at
    BEFORE UPDATE ON trades
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_daily_metrics_updated_at
    BEFORE UPDATE ON daily_metrics
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Função para calcular métricas de performance (otimizada)
CREATE OR REPLACE FUNCTION calculate_win_rate(start_date DATE, end_date DATE)
RETURNS NUMERIC AS $$
DECLARE
    total INTEGER;
    wins INTEGER;
BEGIN
    SELECT COUNT(*), COUNT(*) FILTER (WHERE pnl > 0)
    INTO total, wins
    FROM trades
    WHERE status = 'closed'
      AND DATE(opened_at) BETWEEN start_date AND end_date;
    
    IF total = 0 THEN RETURN 0; END IF;
    RETURN (wins::NUMERIC / total::NUMERIC) * 100;
END;
$$ LANGUAGE plpgsql;

-- ═══════════════════════════════════════════════════════════
-- INITIAL DATA
-- ═══════════════════════════════════════════════════════════

-- Inserir métricas de hoje (será atualizado ao longo do dia)
INSERT INTO daily_metrics (date) 
VALUES (CURRENT_DATE)
ON CONFLICT (date) DO NOTHING;

-- ═══════════════════════════════════════════════════════════
-- COMMENTS PARA DOCUMENTAÇÃO
-- ═══════════════════════════════════════════════════════════

COMMENT ON TABLE trades IS 'Histórico imutável de todas as operações executadas - auditável via Tupã';
COMMENT ON TABLE position_snapshots IS 'Snapshots para reconciliação entre dados locais e Bybit API';
COMMENT ON TABLE system_events IS 'Audit trail de todos os eventos do sistema para debugging e compliance';
COMMENT ON TABLE daily_metrics IS 'Métricas agregadas diárias para dashboard e reporting público';
COMMENT ON TABLE tupa_audit_logs IS 'Logs estruturados do Tupã Engine para auditoria forense e reproducibilidade';
COMMENT ON TABLE profile_history IS 'Histórico de mudanças de perfil de risco para análise de performance';
COMMENT ON TABLE circuit_breaker_events IS 'Registro de ativações de circuit breakers para análise de risco';

COMMENT ON TABLE bybit_fills IS 'Detailed fills received from Bybit for audit and reconciliation';
