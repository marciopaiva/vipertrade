//! Estratégia de swing em 4H — o checklist de entrada, em código.
//!
//! Difere da estratégia de minutos em três pontos que importam:
//!
//! 1. **Horizonte.** Vela de 4H, posição de dias. O custo de ida e volta (0,06%)
//!    consome 1,4% do movimento típico, contra 40% no horizonte de 60min. Foi a
//!    aritmética do custo que inviabilizou a operação de minutos.
//! 2. **Stop estrutural.** Abaixo do fundo real, não a uma distância percentual
//!    fixa. Medido em 167 dias e 22 símbolos: trocar stop de 3% fixo por stop no
//!    fundo levou o resultado de −0,141% para +0,238% por trade, subindo o
//!    acerto de 33,8% para 38,4%. O stop percentual era atingido por ruído.
//! 3. **Risco/retorno explícito.** O alvo é múltiplo da distância até o stop, e
//!    não um percentual fixo. Com R:R 2:1 basta acertar 33% para empatar.
//!
//! Este módulo é puro: sem I/O, sem estado global. Recebe velas e devolve um
//! setup ou nada.

/// Uma vela OHLCV de 4 horas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candle {
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

/// Parâmetros do setup. Os defaults saem da medição de 167 dias.
#[derive(Debug, Clone, Copy)]
pub struct SwingParams {
    /// Períodos da média longa que define a tendência macro.
    pub ema_slow: usize,
    /// Períodos da média curta usada como zona de pullback.
    pub ema_fast: usize,
    /// Quantas velas olhar para trás ao procurar o fundo estrutural.
    ///
    /// O resultado é positivo de 4 a 30 (platô, não pico) — sinal de efeito
    /// real e não de ajuste à amostra. 10 fica no meio.
    pub swing_lookback: usize,
    /// Folga abaixo do fundo, em fração. Evita ser estopado no pavio exato.
    pub stop_margin_pct: f64,
    /// Alvo como múltiplo da distância até o stop.
    ///
    /// 1,5 e não 2,0 desde 2026-08-10. Com 2R apenas 36% dos trades chegavam ao
    /// alvo; a 1,5R sobem para 46%, e o ganho de acerto mais que compensa o
    /// ganho menor por acerto. Medido na régua v6 (sequencial, cooldown, macro
    /// exato, long+short): a faixa 1,0–1,5R é positiva nas DUAS metades do
    /// histórico, enquanto de 1,75R para cima a 1ª metade melhora e a 2ª piora —
    /// assinatura de parâmetro ajustado ao passado.
    ///
    /// 1,5 fica no meio do platô e não na borda: 1,25R mede um pouco melhor
    /// fora da amostra (+0,463% contra +0,399%), mas a diferença é menor que o
    /// ruído e a borda é mais frágil se o mercado mudar.
    pub risk_reward: f64,
    /// Stop mais apertado que isto vira ruído; mais largo, risco demais.
    pub min_stop_pct: f64,
    pub max_stop_pct: f64,
}

impl Default for SwingParams {
    fn default() -> Self {
        Self {
            ema_slow: 200,
            ema_fast: 50,
            swing_lookback: 10,
            stop_margin_pct: 0.005,
            risk_reward: 1.5,
            min_stop_pct: 0.003,
            max_stop_pct: 0.12,
        }
    }
}

/// Um setup aprovado: onde entrar, onde sair no erro e onde realizar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwingSetup {
    pub entry: f64,
    pub stop: f64,
    pub target: f64,
    /// Distância até o stop, em fração do preço de entrada.
    pub risk_pct: f64,
}

/// Média exponencial do último ponto da série. `None` se não há histórico.
pub fn ema(values: &[f64], period: usize) -> Option<f64> {
    if period == 0 || values.len() < period {
        return None;
    }
    let k = 2.0 / (period as f64 + 1.0);
    let mut e = values[0];
    for v in &values[1..] {
        e = v * k + e * (1.0 - k);
    }
    Some(e)
}

/// Fundo estrutural: a menor mínima das últimas `lookback` velas.
///
/// Olha só para trás — é o que torna a regra utilizável em tempo real. Um swing
/// low "confirmado" (mínima com velas maiores dos dois lados) precisaria do
/// futuro para ser identificado.
pub fn structural_low(candles: &[Candle], lookback: usize) -> Option<f64> {
    if candles.is_empty() || lookback == 0 {
        return None;
    }
    let start = candles.len().saturating_sub(lookback);
    candles[start..]
        .iter()
        .map(|c| c.low)
        .fold(None::<f64>, |acc, l| Some(acc.map_or(l, |a: f64| a.min(l))))
}

/// Tendência macro de alta: preço acima da média longa.
pub fn is_uptrend(candles: &[Candle], p: &SwingParams) -> Option<bool> {
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
    let slow = ema(&closes, p.ema_slow)?;
    Some(closes.last()? > &slow)
}

/// Recuo até a média curta — a zona onde se compra numa tendência de alta.
///
/// Comprar no meio do caminho é o erro que o checklist evita: espera-se o
/// pullback até a zona onde compradores defenderam antes.
pub fn is_pullback(candles: &[Candle], p: &SwingParams) -> Option<bool> {
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
    let fast = ema(&closes, p.ema_fast)?;
    Some(closes.last()? <= &fast)
}

/// Vela de martelo: corpo pequeno, pavio inferior longo, pouco pavio superior.
/// Sinaliza que a pressão vendedora foi rejeitada.
pub fn is_hammer(c: &Candle) -> bool {
    let body = (c.close - c.open).abs();
    let lower_wick = c.open.min(c.close) - c.low;
    let upper_wick = c.high - c.open.max(c.close);
    let range = c.high - c.low;
    range > 0.0 && lower_wick >= 2.0 * body && upper_wick <= body
}

/// Engolfo de alta: vela verde que cobre integralmente a vermelha anterior.
pub fn is_bullish_engulfing(prev: &Candle, cur: &Candle) -> bool {
    prev.close < prev.open
        && cur.close > cur.open
        && cur.close >= prev.open
        && cur.open <= prev.close
}

/// Avalia o setup de compra sobre a série (a última vela é a atual).
///
/// `btc_uptrend` vem de fora porque é o filtro macro do checklist: por melhor
/// que esteja o gráfico da alt, não se compra contra o Bitcoin caindo.
///
/// Devolve `None` quando qualquer condição falha — a ausência de setup é o caso
/// normal, não uma exceção.
pub fn evaluate_long(candles: &[Candle], btc_uptrend: bool, p: &SwingParams) -> Option<SwingSetup> {
    if !btc_uptrend {
        return None;
    }
    if candles.len() < p.ema_slow {
        return None;
    }
    if !is_uptrend(candles, p)? || !is_pullback(candles, p)? {
        return None;
    }

    let entry = candles.last()?.close;
    let low = structural_low(candles, p.swing_lookback)?;
    let stop = low * (1.0 - p.stop_margin_pct);
    if stop <= 0.0 || stop >= entry {
        return None;
    }

    let risk_pct = (entry - stop) / entry;
    // Fora da faixa não há setup: stop colado vira ruído, stop largo demais
    // arrisca mais do que o alvo justifica.
    if risk_pct < p.min_stop_pct || risk_pct > p.max_stop_pct {
        return None;
    }

    Some(SwingSetup {
        entry,
        stop,
        target: entry * (1.0 + risk_pct * p.risk_reward),
        risk_pct,
    })
}

/// Topo estrutural: a maior máxima das últimas `lookback` velas.
///
/// Espelho de `structural_low`, para o short. Mesma restrição: só olha para
/// trás, senão a regra dependeria do futuro.
pub fn structural_high(candles: &[Candle], lookback: usize) -> Option<f64> {
    if candles.is_empty() || lookback == 0 {
        return None;
    }
    let start = candles.len().saturating_sub(lookback);
    candles[start..]
        .iter()
        .map(|c| c.high)
        .fold(None::<f64>, |acc, h| Some(acc.map_or(h, |a: f64| a.max(h))))
}

/// Tendência de baixa: preço abaixo da média longa.
pub fn is_downtrend(candles: &[Candle], p: &SwingParams) -> Option<bool> {
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
    let slow = ema(&closes, p.ema_slow)?;
    Some(closes.last()? < &slow)
}

/// Repique: o preço subiu de volta até a média curta.
pub fn is_rally(candles: &[Candle], p: &SwingParams) -> Option<bool> {
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
    let fast = ema(&closes, p.ema_fast)?;
    Some(closes.last()? >= &fast)
}

/// O espelho short do checklist.
///
/// Vale menos que o long (+0,149%/trade contra +0,312% no corpus de 166 dias)
/// e depende do filtro macro INVERTIDO: sem ele o short rende −0,066%. O valor
/// não está no retorno próprio e sim na alternância — long e short operam em
/// regimes opostos, então onde um fica parado o outro trabalha. Medido nas
/// metades do histórico, o long faz 183 trades na primeira e 48 na segunda; o
/// short faz 21 e 155.
pub fn evaluate_short(candles: &[Candle], btc_downtrend: bool, p: &SwingParams) -> Option<SwingSetup> {
    if !btc_downtrend {
        return None;
    }
    if candles.len() < p.ema_slow {
        return None;
    }
    if !is_downtrend(candles, p)? || !is_rally(candles, p)? {
        return None;
    }

    let entry = candles.last()?.close;
    let high = structural_high(candles, p.swing_lookback)?;
    let stop = high * (1.0 + p.stop_margin_pct);
    if stop <= entry {
        return None;
    }

    let risk_pct = (stop - entry) / entry;
    if risk_pct < p.min_stop_pct || risk_pct > p.max_stop_pct {
        return None;
    }

    Some(SwingSetup {
        entry,
        stop,
        target: entry * (1.0 - risk_pct * p.risk_reward),
        risk_pct,
    })
}

/// Estado de cada regra do checklist para um símbolo, no instante da avaliação.
///
/// Existe para a matriz de decisão poder mostrar POR QUE não há entrada — a
/// tela anterior herdou as colunas do scalp (RSI, %B, ADX, consenso) e passou a
/// exibir onze linhas de indicadores que não participam mais de nenhuma decisão.
///
/// Deriva das mesmas funções que decidem (`is_uptrend`, `is_pullback`,
/// `structural_low`), e não de uma cópia da regra: um diagnóstico que pode
/// divergir do motor é pior que nenhum.
#[derive(Debug, Clone, PartialEq)]
pub struct SwingDiagnosis {
    /// Lado que o filtro macro habilita agora: `Long` ou `Short`.
    pub side: &'static str,
    pub price: f64,
    pub ema_slow: Option<f64>,
    pub ema_fast: Option<f64>,
    /// Regra 1 do lado ativo — acima da média longa no long, abaixo no short.
    pub trend_ok: bool,
    /// Regra 2 do lado ativo — recuo à média curta no long, repique no short.
    pub pullback_ok: bool,
    /// Fundo estrutural das últimas `swing_lookback` velas, já com a folga.
    pub stop: Option<f64>,
    /// Distância até o stop, em fração do preço.
    pub risk_pct: Option<f64>,
    /// Regra 3 — o risco cabe na faixa operável.
    pub risk_in_range: bool,
    /// Velas disponíveis; abaixo de `ema_slow` não há como avaliar.
    pub candles: usize,
}

impl SwingDiagnosis {
    /// Rótulo estável do estado, resolvido na ordem em que as regras barram.
    ///
    /// `has_position` e `btc_uptrend` entram aqui porque precedem o checklist:
    /// não adianta dizer "sem recuo" para um símbolo que já está posicionado.
    pub fn status(&self, has_position: bool) -> &'static str {
        if has_position {
            return "position_open";
        }
        if self.candles < 200 {
            return "insufficient_history";
        }
        if !self.trend_ok {
            return "no_uptrend";
        }
        if !self.pullback_ok {
            return "awaiting_pullback";
        }
        if !self.risk_in_range {
            return "risk_out_of_range";
        }
        "setup"
    }
}

/// Avalia o checklist DO LADO ATIVO e devolve o estado de cada regra.
///
/// `long_side` vem do filtro macro: com o BTC acima da própria EMA200 procura-se
/// compra; abaixo, venda. Os dois lados nunca são avaliados ao mesmo tempo — é
/// a alternância que faz o conjunto funcionar, e avaliar ambos produziria uma
/// matriz que sugere entradas que o motor não vai tomar.
pub fn diagnose(candles: &[Candle], long_side: bool, p: &SwingParams) -> SwingDiagnosis {
    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();
    let price = closes.last().copied().unwrap_or(0.0);
    let slow = ema(&closes, p.ema_slow);
    let fast = ema(&closes, p.ema_fast);

    let (trend_ok, pullback_ok, stop) = if long_side {
        (
            is_uptrend(candles, p).unwrap_or(false),
            is_pullback(candles, p).unwrap_or(false),
            structural_low(candles, p.swing_lookback).map(|l| l * (1.0 - p.stop_margin_pct)),
        )
    } else {
        (
            is_downtrend(candles, p).unwrap_or(false),
            is_rally(candles, p).unwrap_or(false),
            structural_high(candles, p.swing_lookback).map(|h| h * (1.0 + p.stop_margin_pct)),
        )
    };

    let risk_pct = stop.and_then(|st| {
        if price <= 0.0 {
            return None;
        }
        if long_side && st < price {
            Some((price - st) / price)
        } else if !long_side && st > price {
            Some((st - price) / price)
        } else {
            None
        }
    });
    let risk_in_range = risk_pct.is_some_and(|r| r >= p.min_stop_pct && r <= p.max_stop_pct);

    SwingDiagnosis {
        side: if long_side { "Long" } else { "Short" },
        price,
        ema_slow: slow,
        ema_fast: fast,
        trend_ok,
        pullback_ok,
        stop,
        risk_pct,
        risk_in_range,
        candles: candles.len(),
    }
}

/// Motivo de saída de uma posição swing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwingExit {
    /// Preço tocou o stop fixo: a tese estrutural foi invalidada.
    StopLoss,
    /// Preço alcançou o alvo: o risco/retorno planejado se realizou.
    TakeProfit,
}

impl SwingExit {
    /// Texto aceito por `trades_close_reason_check`.
    pub fn close_reason(self) -> &'static str {
        match self {
            SwingExit::StopLoss => "stop_loss",
            SwingExit::TakeProfit => "take_profit",
        }
    }
}

/// Decide se uma posição swing deve fechar ao preço corrente.
///
/// Só stop e alvo — nada de trailing, tese ou corte por tempo. A posição foi
/// aberta com uma tese estrutural e sai quando essa tese se confirma ou se
/// invalida, não quando o preço oscila.
///
/// `None` quando nenhum nível foi tocado, que é o caso normal.
pub fn check_exit(
    side: &str,
    current_price: f64,
    stop: Option<f64>,
    target: Option<f64>,
) -> Option<SwingExit> {
    if !(current_price.is_finite() && current_price > 0.0) {
        return None;
    }
    let is_long = side.eq_ignore_ascii_case("Long");

    // Stop antes do alvo: quando ambos são tocados entre dois ticks não há como
    // saber a ordem, e assumir o pior é o único jeito honesto de medir.
    if let Some(s) = stop.filter(|v| v.is_finite() && *v > 0.0) {
        let hit = if is_long {
            current_price <= s
        } else {
            current_price >= s
        };
        if hit {
            return Some(SwingExit::StopLoss);
        }
    }
    if let Some(t) = target.filter(|v| v.is_finite() && *v > 0.0) {
        let hit = if is_long {
            current_price >= t
        } else {
            current_price <= t
        };
        if hit {
            return Some(SwingExit::TakeProfit);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(open: f64, high: f64, low: f64, close: f64) -> Candle {
        Candle {
            open,
            high,
            low,
            close,
            volume: 1.0,
        }
    }

    /// Série em alta com um recuo no fim — o setup que a estratégia procura.
    fn uptrend_with_pullback() -> Vec<Candle> {
        let mut v: Vec<Candle> = (0..250)
            .map(|i| {
                let base = 100.0 + i as f64 * 0.5;
                c(base, base + 1.0, base - 1.0, base + 0.5)
            })
            .collect();
        // Recuo final: cai abaixo da EMA rápida (50) sem perder a lenta (200).
        // Precisa ser fundo o bastante — a EMA rápida acompanha de perto uma
        // tendência longa, então um recuo raso não a cruza.
        let last = v.last().unwrap().close;
        for k in 0..8 {
            let px = last - (k as f64 + 1.0) * 3.0;
            v.push(c(px + 1.5, px + 2.0, px - 1.5, px));
        }
        v
    }

    /// O R:R é decisão medida, não preferência. Se alguém mudar o default sem
    /// refazer a validação, este teste quebra e obriga a justificar.
    #[test]
    fn default_risk_reward_is_the_measured_one() {
        assert!(
            (SwingParams::default().risk_reward - 1.5).abs() < 1e-9,
            "o default validado é 1,5R (platô 1,0-1,5, positivo nas duas metades); \
             de 1,75R para cima a 2ª metade degrada"
        );
    }

    // ── espelho short ─────────────────────────────────────────────
    /// Série em queda com um repique no fim — o setup de venda.
    fn downtrend_with_rally() -> Vec<Candle> {
        let mut v: Vec<Candle> = (0..250)
            .map(|i| {
                let base = 300.0 - i as f64 * 0.5;
                c(base, base + 1.0, base - 1.0, base - 0.5)
            })
            .collect();
        // Repique final: sobe acima da EMA rápida sem recuperar a lenta.
        let last = v.last().unwrap().close;
        for k in 0..8 {
            let px = last + (k as f64 + 1.0) * 3.0;
            v.push(c(px - 1.5, px + 1.5, px - 2.0, px));
        }
        v
    }

    /// O short é o espelho exato: stop ACIMA do topo, alvo ABAIXO da entrada.
    #[test]
    fn short_setup_mirrors_the_long() {
        let p = SwingParams::default();
        let s = evaluate_short(&downtrend_with_rally(), true, &p).expect("setup short");
        assert!(s.stop > s.entry, "stop {} deve ficar acima da entrada {}", s.stop, s.entry);
        assert!(s.target < s.entry, "alvo {} deve ficar abaixo da entrada {}", s.target, s.entry);
        // R:R obedecido no lado certo
        let risco = (s.stop - s.entry) / s.entry;
        let retorno = (s.entry - s.target) / s.entry;
        assert!((retorno / risco - p.risk_reward).abs() < 1e-9);
    }

    /// Sem o macro invertido não há venda — medido, o short sem filtro rende
    /// −0,066%/trade contra +0,149% com ele.
    #[test]
    fn short_needs_the_inverted_macro() {
        let p = SwingParams::default();
        assert!(evaluate_short(&downtrend_with_rally(), false, &p).is_none());
    }

    /// Uma série de ALTA não gera venda, e uma de baixa não gera compra.
    #[test]
    fn the_two_sides_never_fire_on_the_same_series() {
        let p = SwingParams::default();
        assert!(evaluate_short(&uptrend_with_pullback(), true, &p).is_none());
        assert!(evaluate_long(&downtrend_with_rally(), true, &p).is_none());
    }

    /// O topo estrutural olha só para trás, como o fundo.
    #[test]
    fn structural_high_uses_only_the_lookback_window() {
        let v = vec![
            c(10.0, 99.0, 9.0, 10.0),   // pico antigo, fora da janela
            c(10.0, 12.0, 9.0, 11.0),
            c(11.0, 13.0, 10.0, 12.0),
        ];
        assert_eq!(structural_high(&v, 2), Some(13.0));
        assert_eq!(structural_high(&v, 3), Some(99.0));
    }

    // ── diagnóstico do checklist ──────────────────────────────────
    /// O diagnóstico só serve se concordar com quem decide. Se `diagnose` disser
    /// "setup" e `evaluate_long` não devolver nada (ou vice-versa), a tela passa
    /// a mentir sobre o motor — que é pior do que não ter tela.
    #[test]
    fn diagnosis_agrees_with_the_decision() {
        let p = SwingParams::default();
        let series = uptrend_with_pullback();
        for corte in [200usize, 220, 240, series.len()] {
            let janela = &series[..corte.min(series.len())];
            let d = diagnose(janela, true, &p);
            let diz_setup = d.status(false) == "setup";
            let tem_setup = evaluate_long(janela, true, &p).is_some();
            assert_eq!(
                diz_setup, tem_setup,
                "divergência com {corte} velas: diagnóstico={diz_setup} motor={tem_setup}"
            );
        }
    }

    /// Uma posição aberta precede o checklist: dizer "sem recuo" para um símbolo
    /// já posicionado descreve uma avaliação que não vale.
    #[test]
    fn open_position_precedes_every_other_reason() {
        let d = diagnose(&uptrend_with_pullback(), true, &SwingParams::default());
        assert_eq!(d.status(true), "position_open");
        
    }

    /// O macro agora escolhe o LADO em vez de bloquear. Com o BTC em baixa, a
    /// mesma série que era setup de compra deixa de ser — e passa a ser lida
    /// pelas regras de venda, que ela não cumpre.
    #[test]
    fn macro_selects_the_side_instead_of_blocking() {
        let série = uptrend_with_pullback();
        let p = SwingParams::default();
        assert_eq!(diagnose(&série, true, &p).side, "Long");
        assert_eq!(diagnose(&série, true, &p).status(false), "setup");
        let curto = diagnose(&série, false, &p);
        assert_eq!(curto.side, "Short");
        assert_ne!(curto.status(false), "setup");
    }

    /// Série curta não é "sem tendência": é ausência de avaliação.
    #[test]
    fn short_history_is_reported_as_such() {
        let curta: Vec<Candle> = (0..50).map(|i| {
            let b = 100.0 + i as f64;
            c(b, b + 1.0, b - 1.0, b)
        }).collect();
        assert_eq!(diagnose(&curta, true, &SwingParams::default()).status(false),
                   "insufficient_history");
    }

    /// O stop do diagnóstico tem de ser o MESMO que a entrada usaria.
    #[test]
    fn diagnosed_stop_matches_the_setup_stop() {
        let p = SwingParams::default();
        let série = uptrend_with_pullback();
        let d = diagnose(&série, true, &p);
        let setup = evaluate_long(&série, true, &p).expect("setup");
        assert!((d.stop.unwrap() - setup.stop).abs() < 1e-9);
        assert!((d.risk_pct.unwrap() - setup.risk_pct).abs() < 1e-9);
    }

    // ── saída por stop/alvo fixos ─────────────────────────────────
    /// Quando ambos são tocados entre dois ticks não há como saber a ordem.
    /// Assumir o pior é o único jeito honesto de medir — o contrário
    /// superestimaria o resultado sistematicamente.
    #[test]
    fn stop_wins_when_both_levels_are_hit() {
        // Long com stop em 90 e alvo em 120; preço em 85 tocou os dois cenários.
        assert_eq!(
            check_exit("Long", 85.0, Some(90.0), Some(120.0)),
            Some(SwingExit::StopLoss)
        );
    }

    #[test]
    fn long_exits_below_stop_and_above_target() {
        assert_eq!(
            check_exit("Long", 89.9, Some(90.0), Some(120.0)),
            Some(SwingExit::StopLoss)
        );
        assert_eq!(
            check_exit("Long", 90.0, Some(90.0), Some(120.0)),
            Some(SwingExit::StopLoss)
        );
        assert_eq!(
            check_exit("Long", 120.0, Some(90.0), Some(120.0)),
            Some(SwingExit::TakeProfit)
        );
        assert_eq!(check_exit("Long", 100.0, Some(90.0), Some(120.0)), None);
    }

    /// No Short os lados se invertem — trocar isso fecharia toda posição
    /// vendida no lugar errado, e no exato oposto do pretendido.
    #[test]
    fn short_inverts_both_sides() {
        assert_eq!(
            check_exit("Short", 110.0, Some(110.0), Some(80.0)),
            Some(SwingExit::StopLoss)
        );
        assert_eq!(
            check_exit("Short", 80.0, Some(110.0), Some(80.0)),
            Some(SwingExit::TakeProfit)
        );
        assert_eq!(check_exit("Short", 100.0, Some(110.0), Some(80.0)), None);
    }

    #[test]
    fn missing_levels_never_trigger_an_exit() {
        assert_eq!(check_exit("Long", 50.0, None, None), None);
        assert_eq!(check_exit("Long", 50.0, None, Some(120.0)), None);
        // Preço inválido não pode fechar posição.
        assert_eq!(check_exit("Long", 0.0, Some(90.0), Some(120.0)), None);
        assert_eq!(check_exit("Long", f64::NAN, Some(90.0), Some(120.0)), None);
    }

    /// Os textos precisam bater com `trades_close_reason_check` no banco —
    /// divergir aqui faz o INSERT falhar só em produção.
    #[test]
    fn close_reasons_match_the_database_constraint() {
        assert_eq!(SwingExit::StopLoss.close_reason(), "stop_loss");
        assert_eq!(SwingExit::TakeProfit.close_reason(), "take_profit");
    }

    #[test]
    fn structural_low_uses_only_the_lookback_window() {
        let v = vec![
            c(10.0, 11.0, 5.0, 10.0),
            c(10.0, 11.0, 8.0, 10.0),
            c(10.0, 11.0, 9.0, 10.0),
        ];
        // Janela de 2 ignora a mínima de 5.0 que ficou para trás.
        assert_eq!(structural_low(&v, 2), Some(8.0));
        assert_eq!(structural_low(&v, 3), Some(5.0));
        // Janela maior que a série não estoura.
        assert_eq!(structural_low(&v, 99), Some(5.0));
    }

    /// O stop tem de ficar ABAIXO do fundo, senão é estopado pelo pavio que já
    /// aconteceu — era o defeito do stop percentual.
    #[test]
    fn stop_sits_below_the_structural_low() {
        let v = uptrend_with_pullback();
        let s = evaluate_long(&v, true, &SwingParams::default()).expect("setup esperado");
        let low = structural_low(&v, SwingParams::default().swing_lookback).unwrap();
        assert!(
            s.stop < low,
            "stop {} deve ficar abaixo do fundo {}",
            s.stop,
            low
        );
        assert!(s.stop < s.entry);
    }

    /// O alvo é múltiplo do risco — é o que permite ser lucrativo acertando 38%.
    #[test]
    fn target_respects_the_risk_reward_ratio() {
        let v = uptrend_with_pullback();
        let mut p = SwingParams::default();
        p.risk_reward = 2.0;
        let s = evaluate_long(&v, true, &p).expect("setup esperado");

        let risk = s.entry - s.stop;
        let reward = s.target - s.entry;
        assert!(
            (reward / risk - 2.0).abs() < 1e-9,
            "esperado 2:1, veio {:.3}:1",
            reward / risk
        );
    }

    /// A regra número um do checklist: não comprar altcoin contra o BTC caindo.
    #[test]
    fn btc_downtrend_blocks_every_setup() {
        let v = uptrend_with_pullback();
        assert!(evaluate_long(&v, true, &SwingParams::default()).is_some());
        assert!(
            evaluate_long(&v, false, &SwingParams::default()).is_none(),
            "BTC em queda tem de bloquear a entrada"
        );
    }

    #[test]
    fn no_setup_without_enough_history() {
        let v: Vec<Candle> = (0..50)
            .map(|i| c(100.0, 101.0, 99.0, 100.0 + i as f64))
            .collect();
        assert!(evaluate_long(&v, true, &SwingParams::default()).is_none());
    }

    /// Stop fora da faixa não vira setup: colado demais é ruído, largo demais
    /// arrisca mais do que o alvo compensa.
    #[test]
    fn rejects_stops_outside_the_usable_band() {
        let v = uptrend_with_pullback();
        let mut p = SwingParams::default();

        p.min_stop_pct = 0.90; // exige risco absurdo
        assert!(evaluate_long(&v, true, &p).is_none());

        p = SwingParams::default();
        p.max_stop_pct = 0.0001; // nenhum stau real cabe
        assert!(evaluate_long(&v, true, &p).is_none());
    }

    #[test]
    fn hammer_needs_a_long_lower_wick() {
        // Corpo pequeno no topo, pavio inferior longo.
        assert!(is_hammer(&c(10.0, 10.2, 8.0, 10.1)));
        // Vela cheia sem pavio: não é martelo.
        assert!(!is_hammer(&c(8.0, 10.2, 8.0, 10.1)));
        // Pavio para cima: rejeição de alta, não de baixa.
        assert!(!is_hammer(&c(10.0, 12.0, 9.9, 10.1)));
    }

    #[test]
    fn engulfing_requires_covering_the_previous_body() {
        let red = c(10.0, 10.1, 9.0, 9.2);
        assert!(is_bullish_engulfing(&red, &c(9.1, 10.3, 9.0, 10.1)));
        // Verde que não cobre o corpo anterior.
        assert!(!is_bullish_engulfing(&red, &c(9.1, 9.6, 9.0, 9.5)));
        // Anterior verde: não há o que engolir.
        assert!(!is_bullish_engulfing(
            &c(9.0, 10.0, 8.9, 9.8),
            &c(9.1, 10.3, 9.0, 10.1)
        ));
    }

    #[test]
    fn ema_needs_enough_points() {
        assert!(ema(&[1.0, 2.0], 5).is_none());
        assert!(ema(&[], 1).is_none());
        // Série constante: a média é o próprio valor.
        let flat = vec![10.0; 30];
        assert!((ema(&flat, 20).unwrap() - 10.0).abs() < 1e-9);
    }
}
