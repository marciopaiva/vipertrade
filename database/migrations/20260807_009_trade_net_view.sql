-- View com o resultado LÍQUIDO de cada trade fechado.
--
-- A fórmula (pnl - fees - funding) estava repetida em cada agregado da API, e
-- foi essa repetição que deixou o painel de qualidade somando PnL bruto por
-- meses depois de as taxas passarem a ser cobradas: quem escreveu a query nova
-- copiou a antiga. Com a view há um lugar só para acertar.
--
-- `net_pnl_pct` repete a fórmula que o executor grava em `trades.pnl_pct` (que
-- já é líquida) em vez de ler a coluna, para que trades fechados antes daquela
-- correção também apareçam certos.
CREATE OR REPLACE VIEW trade_net AS
SELECT
    trade_id,
    symbol,
    side,
    COALESCE(strategy_kind, 'scalp')                    AS strategy_kind,
    opened_at,
    closed_at,
    close_reason,
    COALESCE(mfe_pct, 0)                                AS mfe_pct,
    COALESCE(mae_pct, 0)                                AS mae_pct,
    (entry_price * quantity)::double precision          AS notional,
    (COALESCE(fees, 0) + COALESCE(funding_paid, 0))::double precision AS cost,
    (COALESCE(pnl, 0) - COALESCE(fees, 0) - COALESCE(funding_paid, 0))::double precision AS net_pnl,
    (CASE
        WHEN entry_price > 0 THEN
            (COALESCE(pnl, 0) - COALESCE(fees, 0) - COALESCE(funding_paid, 0))
            / (entry_price * quantity) * 100
        ELSE 0
     END)::double precision                             AS net_pnl_pct
FROM trades
WHERE status = 'closed'
  AND paper_trade = TRUE;
