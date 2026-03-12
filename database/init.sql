-- database/init.sql
-- ViperTrade - Initial Data Setup

-- ═══════════════════════════════════════════════════════════
-- CONFIGURAÇÕES INICIAIS
-- ═══════════════════════════════════════════════════════════

-- Configurar timezone para UTC (consistência em trading)
SET timezone = 'UTC';

-- Configurar statement timeout para queries longas (backtest)
SET statement_timeout = '300s';

-- ═══════════════════════════════════════════════════════════
-- INSERIR DADOS DE REFERÊNCIA
-- ═══════════════════════════════════════════════════════════

-- Pares de trading suportados (para validação)
CREATE TABLE IF NOT EXISTS trading_pairs_config (
    symbol TEXT PRIMARY KEY,
    category TEXT NOT NULL,
    min_order_value_usdt NUMERIC NOT NULL,
    max_order_value_usdt NUMERIC NOT NULL,
    price_precision INTEGER NOT NULL,
    qty_precision INTEGER NOT NULL,
    tick_size NUMERIC NOT NULL,
    step_size NUMERIC NOT NULL,
    min_24h_volume_usdt NUMERIC NOT NULL,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

INSERT INTO trading_pairs_config (symbol, category, min_order_value_usdt, max_order_value_usdt, price_precision, qty_precision, tick_size, step_size, min_24h_volume_usdt) VALUES
    ('DOGEUSDT', 'meme_coin', 5.00, 50.00, 5, 0, 0.00001, 1, 50000000),
    ('XRPUSDT', 'large_cap_alt', 5.00, 50.00, 4, 1, 0.0001, 1, 100000000),
    ('ADAUSDT', 'large_cap_alt', 5.00, 50.00, 4, 0, 0.0001, 1, 50000000),
    ('XLMUSDT', 'large_cap_alt', 5.00, 50.00, 5, 0, 0.00001, 1, 60000000)
ON CONFLICT (symbol) DO NOTHING;

-- Perfis de risco com parâmetros
CREATE TABLE IF NOT EXISTS risk_profiles (
    profile_code TEXT PRIMARY KEY CHECK (profile_code IN ('CONSERVATIVE', 'MEDIUM', 'AGGRESSIVE')),
    risk_per_trade_pct NUMERIC NOT NULL,
    stop_loss_pct NUMERIC NOT NULL,
    take_profit_pct NUMERIC NOT NULL,
    max_leverage NUMERIC NOT NULL,
    max_daily_loss_pct NUMERIC NOT NULL,
    max_open_positions INTEGER NOT NULL,
    max_total_exposure_pct NUMERIC NOT NULL,
    atr_multiplier NUMERIC NOT NULL,
    trailing_config JSONB NOT NULL,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

INSERT INTO risk_profiles (profile_code, risk_per_trade_pct, stop_loss_pct, take_profit_pct, max_leverage, max_daily_loss_pct, max_open_positions, max_total_exposure_pct, atr_multiplier, trailing_config) VALUES
    ('CONSERVATIVE', 0.75, 0.01, 0.02, 2, 0.02, 1, 0.30, 0.3, '{"activate_after_profit_pct": 0.01, "initial_trail_pct": 0.005, "ratchet_levels": [{"at_profit_pct": 0.02, "trail_pct": 0.008}, {"at_profit_pct": 0.04, "trail_pct": 0.012}, {"at_profit_pct": 0.06, "trail_pct": 0.02}], "move_to_break_even_at": 0.015}'),
    ('MEDIUM', 1.25, 0.015, 0.03, 2, 0.03, 2, 0.50, 0.5, '{"activate_after_profit_pct": 0.015, "initial_trail_pct": 0.008, "ratchet_levels": [{"at_profit_pct": 0.03, "trail_pct": 0.012}, {"at_profit_pct": 0.06, "trail_pct": 0.02}, {"at_profit_pct": 0.10, "trail_pct": 0.035}], "move_to_break_even_at": 0.02}'),
    ('AGGRESSIVE', 2.00, 0.025, 0.05, 3, 0.05, 3, 0.70, 0.7, '{"activate_after_profit_pct": 0.02, "initial_trail_pct": 0.01, "ratchet_levels": [{"at_profit_pct": 0.05, "trail_pct": 0.02}, {"at_profit_pct": 0.10, "trail_pct": 0.035}, {"at_profit_pct": 0.15, "trail_pct": 0.05}], "move_to_break_even_at": 0.03}')
ON CONFLICT (profile_code) DO UPDATE SET 
    risk_per_trade_pct = EXCLUDED.risk_per_trade_pct,
    updated_at = NOW();

-- ═══════════════════════════════════════════════════════════
-- VIEWS PARA CONSULTAS COMUNS
-- ═══════════════════════════════════════════════════════════

-- View para performance pública (para followers verem)
CREATE OR REPLACE VIEW public_performance AS
SELECT 
    CURRENT_DATE as date,
    COUNT(*) FILTER (WHERE status = 'closed' AND DATE(opened_at) >= CURRENT_DATE - INTERVAL '30 days') as trades_30d,
    COUNT(*) FILTER (WHERE status = 'closed' AND pnl > 0 AND DATE(opened_at) >= CURRENT_DATE - INTERVAL '30 days') as winning_trades_30d,
    ROUND(COUNT(*) FILTER (WHERE status = 'closed' AND pnl > 0 AND DATE(opened_at) >= CURRENT_DATE - INTERVAL '30 days')::NUMERIC / 
          NULLIF(COUNT(*) FILTER (WHERE status = 'closed' AND DATE(opened_at) >= CURRENT_DATE - INTERVAL '30 days'), 0) * 100, 2) as win_rate_30d,
    ROUND(SUM(pnl) FILTER (WHERE status = 'closed' AND DATE(opened_at) >= CURRENT_DATE - INTERVAL '30 days'), 2) as pnl_30d,
    ROUND(SUM(pnl) FILTER (WHERE status = 'closed' AND DATE(opened_at) >= CURRENT_DATE - INTERVAL '30 days') / 
          NULLIF(SUM(ABS(pnl)) FILTER (WHERE status = 'closed' AND DATE(opened_at) >= CURRENT_DATE - INTERVAL '30 days'), 0), 2) as profit_factor_30d,
    ROUND(MIN(pnl_pct) FILTER (WHERE status = 'closed' AND DATE(opened_at) >= CURRENT_DATE - INTERVAL '30 days'), 2) as max_drawdown_30d,
    ROUND(AVG(duration_seconds) FILTER (WHERE status = 'closed' AND DATE(opened_at) >= CURRENT_DATE - INTERVAL '30 days') / 60, 1) as avg_trade_duration_minutes
FROM trades
WHERE paper_trade = false;  -- Excluir paper trades da performance pública

-- View para reconciliação pendente
CREATE OR REPLACE VIEW pending_reconciliations AS
SELECT 
    ps.symbol,
    ps.snapshot_at,
    ps.divergence_pct,
    ps.bybit_data->>'unrealized_pnl' as bybit_pnl,
    ps.local_calculation->>'unrealized_pnl' as local_pnl,
    ABS((ps.bybit_data->>'unrealized_pnl')::NUMERIC - (ps.local_calculation->>'unrealized_pnl')::NUMERIC) as pnl_diff
FROM position_snapshots ps
WHERE ps.reconciled = false
  AND ps.divergence_pct > 0.01  -- Apenas divergências significativas
ORDER BY ps.divergence_pct DESC, ps.snapshot_at DESC;

-- ═══════════════════════════════════════════════════════════
-- PERMISSÕES (para multi-user no futuro)
-- ═══════════════════════════════════════════════════════════

-- Criar role para aplicação (se não existir)
DO $$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'viper_app') THEN
        CREATE ROLE viper_app WITH LOGIN PASSWORD 'change_me_in_production';
    END IF;
END
$$;

-- Grant permissions
GRANT CONNECT ON DATABASE vipertrade TO viper_app;
GRANT USAGE ON SCHEMA public TO viper_app;
GRANT SELECT, INSERT, UPDATE ON ALL TABLES IN SCHEMA public TO viper_app;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO viper_app;

-- ═══════════════════════════════════════════════════════════
-- ANÁLISE INICIAL PARA PERFORMANCE
-- ═══════════════════════════════════════════════════════════

ANALYZE trades;
ANALYZE position_snapshots;
ANALYZE system_events;
ANALYZE daily_metrics;
ANALYZE tupa_audit_logs;