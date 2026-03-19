ALTER TABLE trades
DROP CONSTRAINT IF EXISTS trades_close_reason_check;

ALTER TABLE trades
ADD CONSTRAINT trades_close_reason_check
CHECK (
    close_reason IN (
        'take_profit',
        'stop_loss',
        'trailing_stop',
        'time_exit',
        'manual',
        'liquidation',
        'circuit_breaker',
        'thesis_invalidated',
        'error'
    )
);
