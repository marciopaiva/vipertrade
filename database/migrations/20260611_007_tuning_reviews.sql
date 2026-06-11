-- Periodic "tuning review" snapshots written by ai-analyst every N closed
-- trades: the AI agent's evidence (close_reason / by_symbol breakdown), the
-- deterministic backtest sweeps it ran, and its guardrail-verified
-- recommendation. Read by the api (/api/v1/tuning-reviews) for the /analysis feed.
CREATE TABLE IF NOT EXISTS tuning_reviews (
    id           BIGSERIAL PRIMARY KEY,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    trade_count  INTEGER     NOT NULL,
    review       JSONB       NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_tuning_reviews_created
    ON tuning_reviews (created_at DESC);
