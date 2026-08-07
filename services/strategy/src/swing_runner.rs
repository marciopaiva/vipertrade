//! Consumidor das velas de 4H: transforma série em decisão de entrada.
//!
//! Roda em paralelo ao loop de scalp, com estado próprio. As duas famílias não
//! compartilham nada além do banco e do stream de decisões — o isolamento na
//! gestão de posição está no loop principal (`OpenTradeSnapshot::is_swing`).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tracing::{error, info, warn};
use viper_domain::{
    stream_ensure_group, OhlcCandle, StrategyDecision, SwingCandlesEvent,
    REDIS_STREAM_SWING_CANDLES,
};

use crate::swing::{self, Candle, SwingParams};

const GROUP: &str = "swing";
/// O BTC é coletado para o filtro macro, nunca operado.
const MACRO_SYMBOL: &str = "BTCUSDT";

/// Série de 4H mais recente por símbolo.
pub(crate) type CandleStore = Arc<Mutex<HashMap<String, Vec<Candle>>>>;

fn to_candles(raw: &[OhlcCandle]) -> Vec<Candle> {
    raw.iter()
        .map(|c| Candle {
            open: c.open,
            high: c.high,
            low: c.low,
            close: c.close,
            volume: c.volume,
        })
        .collect()
}

/// Marcador da família no `reason`.
///
/// O executor identifica a origem por aqui, como já faz com `close_reason`.
/// Passar por um campo novo na `StrategyDecision` quebraria as fixtures de
/// contrato sem ganho real.
pub const SWING_ENTRY_PREFIX: &str = "swing_entry";

/// Monta a decisão de entrada com stop e alvo já definidos.
///
/// `quantity` sai do risco: arriscar uma fração fixa do capital por trade é o
/// que torna o R:R comparável entre símbolos — sem isso, um ativo volátil
/// arriscaria muito mais que um calmo com a mesma posição em dólar.
pub fn build_entry(
    symbol: &str,
    setup: &swing::SwingSetup,
    equity_usdt: f64,
    risk_per_trade: f64,
    leverage: f64,
) -> Option<StrategyDecision> {
    if setup.risk_pct <= 0.0 || equity_usdt <= 0.0 {
        return None;
    }
    let risk_budget = equity_usdt * risk_per_trade;
    let notional = risk_budget / setup.risk_pct;
    let quantity = notional / setup.entry;
    if !(quantity.is_finite() && quantity > 0.0) {
        return None;
    }

    Some(StrategyDecision {
        action: "ENTER_LONG".to_string(),
        symbol: symbol.to_string(),
        quantity,
        leverage,
        entry_price: setup.entry,
        stop_loss: setup.stop,
        take_profit: setup.target,
        reason: format!(
            "{}_stop_{:.4}_target_{:.4}_risk_{:.3}pct",
            SWING_ENTRY_PREFIX,
            setup.stop,
            setup.target,
            setup.risk_pct * 100.0
        ),
        smart_copy_compatible: true,
    })
}

/// Lê o stream de velas de 4H e mantém a série por símbolo.
///
/// Só armazena — quem decide é `evaluate_symbols`, que roda na cadência dele.
/// Separar as duas coisas evita avaliar 11 símbolos a cada mensagem recebida.
pub(crate) async fn run_candle_reader(
    redis_url: String,
    store: CandleStore,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    let consumer = format!("swing-{}", std::process::id());
    loop {
        let client = match redis::Client::open(redis_url.as_str()) {
            Ok(c) => c,
            Err(e) => {
                error!(error = %e, "Swing reader: Redis client failed");
                tokio::time::sleep(Duration::from_secs(3)).await;
                continue;
            }
        };
        let mut conn = match client.get_multiplexed_async_connection().await {
            Ok(c) => c,
            Err(e) => {
                error!(error = %e, "Swing reader: Redis connection failed");
                tokio::time::sleep(Duration::from_secs(3)).await;
                continue;
            }
        };
        stream_ensure_group(&mut conn, REDIS_STREAM_SWING_CANDLES, GROUP).await;
        info!("Starting 4H candle reader");

        loop {
            tokio::select! {
                _ = shutdown.changed() => return,
                result = async {
                    let r: redis::RedisResult<viper_domain::StreamEntries> = redis::cmd("XREADGROUP")
                        .arg("GROUP").arg(GROUP).arg(&consumer)
                        .arg("BLOCK").arg(5000)
                        .arg("COUNT").arg(10)
                        .arg("STREAMS").arg(REDIS_STREAM_SWING_CANDLES).arg(">")
                        .query_async(&mut conn).await;
                    r
                } => {
                    match result {
                        Ok(entries) => {
                            for (_s, messages) in entries {
                                for (msg_id, fields) in messages {
                                    for (k, v) in fields {
                                        if k != "payload" { continue; }
                                        match serde_json::from_str::<SwingCandlesEvent>(&v) {
                                            Ok(ev) => {
                                                let mut g = store.lock().await;
                                                g.insert(ev.symbol.clone(), to_candles(&ev.candles));
                                            }
                                            Err(e) => warn!(error = %e, "Invalid swing candles payload"),
                                        }
                                        let _: Result<String, _> = redis::cmd("XACK")
                                            .arg(REDIS_STREAM_SWING_CANDLES).arg(GROUP).arg(&msg_id)
                                            .query_async(&mut conn).await;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            warn!(error = %e, "Swing reader: XREADGROUP failed");
                            tokio::time::sleep(Duration::from_secs(2)).await;
                            break;
                        }
                    }
                }
            }
        }
    }
}

/// Símbolos com setup válido no momento.
///
/// Devolve as decisões em vez de publicá-las: publicar é responsabilidade de
/// quem tem a conexão, e separar deixa esta função testável sem Redis.
pub fn evaluate_symbols(
    store: &HashMap<String, Vec<Candle>>,
    open_symbols: &[String],
    equity_usdt: f64,
    risk_per_trade: f64,
    leverage: f64,
    params: &SwingParams,
) -> Vec<StrategyDecision> {
    // Filtro macro: sem série do BTC não há como saber, e na dúvida não se opera.
    let btc_uptrend = match store.get(MACRO_SYMBOL) {
        Some(c) => swing::is_uptrend(c, params).unwrap_or(false),
        None => {
            warn!("No BTC 4H series yet — macro filter unavailable, skipping entries");
            return Vec::new();
        }
    };
    if !btc_uptrend {
        return Vec::new();
    }

    let mut out = Vec::new();
    for (symbol, candles) in store {
        if symbol == MACRO_SYMBOL {
            continue;
        }
        // Uma posição por símbolo, como no scalp.
        if open_symbols.iter().any(|s| s == symbol) {
            continue;
        }
        if let Some(setup) = swing::evaluate_long(candles, btc_uptrend, params) {
            if let Some(d) = build_entry(symbol, &setup, equity_usdt, risk_per_trade, leverage) {
                out.push(d);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(entry: f64, stop: f64, rr: f64) -> swing::SwingSetup {
        let risk_pct = (entry - stop) / entry;
        swing::SwingSetup {
            entry,
            stop,
            target: entry * (1.0 + risk_pct * rr),
            risk_pct,
        }
    }

    /// A posição sai do RISCO, não de um valor fixo em dólar: é o que torna o
    /// R:R comparável entre um ativo volátil e um calmo.
    #[test]
    fn quantity_comes_from_the_risk_budget() {
        // Risco de 1% sobre 1000 = $10. Stop a 5% => notional de $200.
        let s = setup(100.0, 95.0, 2.0);
        let d = build_entry("APTUSDT", &s, 1000.0, 0.01, 2.0).expect("decisão");
        let notional = d.quantity * d.entry_price;
        assert!((notional - 200.0).abs() < 1e-6, "notional {notional}");
    }

    /// Stop mais distante tem de gerar posição MENOR — mesmo risco em dólar.
    #[test]
    fn wider_stop_yields_smaller_position() {
        let apertado = build_entry("A", &setup(100.0, 98.0, 2.0), 1000.0, 0.01, 2.0).unwrap();
        let largo = build_entry("A", &setup(100.0, 90.0, 2.0), 1000.0, 0.01, 2.0).unwrap();
        assert!(
            largo.quantity < apertado.quantity,
            "stop largo deveria reduzir a posição"
        );
    }

    #[test]
    fn entry_carries_stop_and_target() {
        let s = setup(100.0, 95.0, 2.0);
        let d = build_entry("APTUSDT", &s, 1000.0, 0.01, 2.0).unwrap();
        assert_eq!(d.action, "ENTER_LONG");
        assert!((d.stop_loss - 95.0).abs() < 1e-9);
        assert!((d.take_profit - s.target).abs() < 1e-9);
        assert!(
            d.reason.starts_with(SWING_ENTRY_PREFIX),
            "reason: {}",
            d.reason
        );
    }

    #[test]
    fn rejects_degenerate_inputs() {
        let s = setup(100.0, 95.0, 2.0);
        assert!(build_entry("A", &s, 0.0, 0.01, 2.0).is_none());
        let zero_risk = swing::SwingSetup {
            entry: 100.0,
            stop: 100.0,
            target: 110.0,
            risk_pct: 0.0,
        };
        assert!(build_entry("A", &zero_risk, 1000.0, 0.01, 2.0).is_none());
    }

    /// Sem série do BTC o filtro macro não existe. Operar assim seria ignorar
    /// a primeira regra do checklist sem nenhum aviso.
    #[test]
    fn no_entries_without_the_btc_series() {
        let mut store = HashMap::new();
        store.insert("APTUSDT".to_string(), vec![]);
        let out = evaluate_symbols(&store, &[], 1000.0, 0.01, 2.0, &SwingParams::default());
        assert!(out.is_empty());
    }

    #[test]
    fn skips_symbols_that_already_have_a_position() {
        let store: HashMap<String, Vec<Candle>> = HashMap::new();
        let out = evaluate_symbols(
            &store,
            &["APTUSDT".to_string()],
            1000.0,
            0.01,
            2.0,
            &SwingParams::default(),
        );
        assert!(out.is_empty());
    }
}
