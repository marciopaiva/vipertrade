-- Migration rollback: executor fills persistence + strong idempotency
-- Date: 2026-03-08

DROP INDEX IF EXISTS uq_system_events_executor_source_event;
DROP TABLE IF EXISTS bybit_fills;
