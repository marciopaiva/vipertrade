-- Stop e alvo fixos da estratégia de swing (4H).
--
-- A estratégia de minutos gerencia a saída com trailing: o stop se move a cada
-- tick e por isso vive em `trailing_stop_peak_price` + a geometria da config.
-- O swing é o oposto — stop e alvo são definidos NA ENTRADA, a partir do fundo
-- estrutural, e não se movem. Sem persisti-los não há como reconstruir a
-- decisão: recalcular o fundo a cada tick daria um stop diferente conforme
-- novas velas chegam, que é justamente o que a estratégia não quer.
--
-- `strategy_kind` separa as duas famílias. Sem isso, as métricas se misturam e
-- nenhuma das duas avaliações presta — trades de swing e de scalping têm
-- horizonte, custo e expectativa completamente diferentes.

ALTER TABLE trades
  ADD COLUMN IF NOT EXISTS strategy_kind TEXT NOT NULL DEFAULT 'scalp',
  ADD COLUMN IF NOT EXISTS planned_stop_price DOUBLE PRECISION,
  ADD COLUMN IF NOT EXISTS planned_target_price DOUBLE PRECISION;

-- Índice parcial: as consultas de swing sempre filtram por este valor, e a
-- esmagadora maioria das linhas é 'scalp'.
CREATE INDEX IF NOT EXISTS idx_trades_strategy_kind
  ON trades (strategy_kind)
  WHERE strategy_kind <> 'scalp';

COMMENT ON COLUMN trades.strategy_kind IS
  'Família da estratégia que abriu a posição: scalp (minutos, saída por trailing) ou swing (4H, saída por stop/alvo fixos). Métricas das duas não devem ser somadas.';

COMMENT ON COLUMN trades.planned_stop_price IS
  'Stop definido na entrada, abaixo do fundo estrutural. Fixo — não se move como o trailing. NULL em posições scalp.';

COMMENT ON COLUMN trades.planned_target_price IS
  'Alvo definido na entrada como múltiplo da distância até o stop (risco/retorno). Fixo. NULL em posições scalp.';
