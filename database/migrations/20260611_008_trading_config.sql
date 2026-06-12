-- Versioned trading config moved out of pairs.yaml into the DB.
-- Phase 1: the strategy + ai-analyst load the ACTIVE version (seeded from the
-- baked pairs.yaml on first cold boot). Behavior-preserving; editing/hot-reload/
-- apply come in later phases. `config` holds the full pairs.yaml-equivalent JSON
-- (global + each <SYMBOL> block — tunables and the token universe together).
CREATE TABLE IF NOT EXISTS trading_config_versions (
    id               BIGSERIAL PRIMARY KEY,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by       TEXT NOT NULL,                            -- 'migration' | 'operator' | 'ai'
    source_review_id BIGINT REFERENCES tuning_reviews(id),     -- the "why" (Phase 4)
    parent_id        BIGINT REFERENCES trading_config_versions(id),
    note             TEXT,
    config           JSONB NOT NULL,
    active           BOOLEAN NOT NULL DEFAULT false
);

-- At most one active version at a time.
CREATE UNIQUE INDEX IF NOT EXISTS idx_trading_config_one_active
    ON trading_config_versions (active) WHERE active;

CREATE INDEX IF NOT EXISTS idx_trading_config_created
    ON trading_config_versions (created_at DESC);
