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
            risk_reward: 2.0,
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
