-- Migration: replace TRXUSDT support with ADAUSDT
-- Date: 2026-03-12

ALTER TABLE trades
    DROP CONSTRAINT IF EXISTS trades_symbol_check,
    ADD CONSTRAINT trades_symbol_check CHECK (symbol IN ('DOGEUSDT', 'XRPUSDT', 'ADAUSDT', 'XLMUSDT')) NOT VALID;

ALTER TABLE position_snapshots
    DROP CONSTRAINT IF EXISTS position_snapshots_symbol_check,
    ADD CONSTRAINT position_snapshots_symbol_check CHECK (symbol IN ('DOGEUSDT', 'XRPUSDT', 'ADAUSDT', 'XLMUSDT')) NOT VALID;

ALTER TABLE bybit_fills
    DROP CONSTRAINT IF EXISTS bybit_fills_symbol_check,
    ADD CONSTRAINT bybit_fills_symbol_check CHECK (symbol IN ('DOGEUSDT', 'XRPUSDT', 'ADAUSDT', 'XLMUSDT')) NOT VALID;

UPDATE trading_pairs_config
SET symbol = 'ADAUSDT',
    category = 'large_cap_alt',
    price_precision = 4,
    qty_precision = 0,
    tick_size = 0.0001,
    step_size = 1,
    min_24h_volume_usdt = 50000000
WHERE symbol = 'TRXUSDT';
