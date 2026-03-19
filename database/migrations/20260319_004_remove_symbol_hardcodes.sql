-- Migration: remove fixed symbol constraints to allow config-driven token expansion

ALTER TABLE trades
    DROP CONSTRAINT IF EXISTS trades_symbol_check;

ALTER TABLE position_snapshots
    DROP CONSTRAINT IF EXISTS position_snapshots_symbol_check;

ALTER TABLE bybit_fills
    DROP CONSTRAINT IF EXISTS bybit_fills_symbol_check;
