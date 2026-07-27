-- Excursão de preço dentro do trade (MAE / MFE).
--
-- Até aqui só o fechamento era registrado, então não havia como responder a
-- pergunta central do tuning: um stop mais apertado corta os trades que morrem
-- sem matar os que dão certo? A resposta depende de por onde o preço passou, e
-- isso não existia em lugar nenhum.
--
-- `trailing_stop_peak_price` não serve: ele só passa a ser preenchido depois que
-- o trailing arma, então nos trades que nunca armaram vale exatamente o preço de
-- entrada — zero por construção, não por observação.
--
-- Ambos em PONTOS PERCENTUAIS e sempre positivos, medidos a favor da direção da
-- posição:
--   mfe_pct = maior avanço favorável já visto (Maximum Favorable Excursion)
--   mae_pct = maior recuo adverso já visto  (Maximum Adverse Excursion)

ALTER TABLE trades
  ADD COLUMN IF NOT EXISTS mfe_pct DOUBLE PRECISION NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS mae_pct DOUBLE PRECISION NOT NULL DEFAULT 0;

COMMENT ON COLUMN trades.mfe_pct IS
  'Maximum Favorable Excursion: maior avanço a favor da posição, em %, desde a entrada. Rastreado a cada tick, independente de o trailing ter armado.';

COMMENT ON COLUMN trades.mae_pct IS
  'Maximum Adverse Excursion: maior recuo contra a posição, em %, desde a entrada. Positivo. Usado para dimensionar o stop com dado em vez de estimativa.';
