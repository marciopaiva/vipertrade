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
    SwingDiagnosticsSnapshot, SwingSymbolDiagnostic, REDIS_STREAM_SWING_CANDLES,
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

/// Duração da vela em segundos — a estratégia opera 4H.
const CANDLE_SECS: u64 = 4 * 3600;

/// A vela de 4H a que um instante pertence.
pub fn candle_bucket(unix_secs: u64) -> u64 {
    unix_secs / CANDLE_SECS
}

/// Chave de idempotência do setup: UM por símbolo por vela de 4H.
///
/// O executor deduplica por `source_event_id`. Publicar uma constante
/// (`"swing-4h"`) fez a primeira entrada reivindicar a chave e todas as
/// seguintes — de qualquer símbolo, para sempre — serem descartadas como
/// duplicata: 16h de operação com um único trade aberto enquanto o AVAXUSDT
/// reapresentava o mesmo setup a cada 60s.
///
/// Amarrar a chave à vela também alinha o live ao backtest, que avalia uma vez
/// por vela. O avaliador roda a cada 60s para reagir rápido quando a vela vira,
/// não para dar 240 chances ao mesmo setup.
pub fn swing_event_id(symbol: &str, bucket: u64) -> String {
    format!("swing-4h:{symbol}:{bucket}")
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
    max_notional_usdt: f64,
) -> Option<StrategyDecision> {
    if setup.risk_pct <= 0.0 || equity_usdt <= 0.0 {
        return None;
    }
    let risk_budget = equity_usdt * risk_per_trade;
    // O teto é obrigatório: com risco de 3% e stop de 2,4%, a fórmula sozinha
    // pede notional de 124% do capital. Arriscar uma fração fixa dimensiona a
    // posição, mas não a limita — quanto mais apertado o stop, maior a posição.
    let notional = (risk_budget / setup.risk_pct).min(max_notional_usdt.max(0.0));
    if notional <= 0.0 {
        return None;
    }
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

/// Monta o snapshot do checklist para a matriz de decisão.
///
/// Roda no MESMO ciclo que `evaluate_symbols` e a partir do mesmo store, para
/// que a tela não possa discordar do motor. O alvo entra aqui já calculado
/// porque é o que o operador compara com o preço — recalculá-lo no front seria
/// espalhar a regra do R:R por outra linguagem.
pub fn diagnose_symbols(
    store: &HashMap<String, Vec<Candle>>,
    open_symbols: &[String],
    params: &SwingParams,
) -> SwingDiagnosticsSnapshot {
    let btc = store.get(MACRO_SYMBOL);
    let btc_diag = btc.map(|c| swing::diagnose(c, params));
    let btc_uptrend = btc
        .and_then(|c| swing::is_uptrend(c, params))
        .unwrap_or(false);

    let mut symbols: Vec<SwingSymbolDiagnostic> = store
        .iter()
        .filter(|(s, _)| s.as_str() != MACRO_SYMBOL)
        .map(|(symbol, candles)| {
            let d = swing::diagnose(candles, params);
            let has_position = open_symbols.iter().any(|s| s == symbol);
            let target = d
                .risk_pct
                .map(|r| d.price * (1.0 + r * params.risk_reward));
            SwingSymbolDiagnostic {
                symbol: symbol.clone(),
                price: d.price,
                ema_slow: d.ema_slow,
                ema_fast: d.ema_fast,
                uptrend: d.uptrend,
                pullback: d.pullback,
                stop: d.stop,
                target,
                risk_pct: d.risk_pct,
                risk_in_range: d.risk_in_range,
                has_position,
                candles: d.candles,
                status: d.status(has_position, btc_uptrend).to_string(),
            }
        })
        .collect();
    symbols.sort_by(|a, b| a.symbol.cmp(&b.symbol));

    SwingDiagnosticsSnapshot {
        schema_version: viper_domain::SCHEMA_VERSION.to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        btc_uptrend,
        btc_price: btc_diag.as_ref().map(|d| d.price),
        btc_ema_slow: btc_diag.as_ref().and_then(|d| d.ema_slow),
        symbols,
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
    max_notional_usdt: f64,
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
            if let Some(d) = build_entry(
                symbol,
                &setup,
                equity_usdt,
                risk_per_trade,
                leverage,
                max_notional_usdt,
            ) {
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
        let d = build_entry("APTUSDT", &s, 1000.0, 0.01, 2.0, 1e9).expect("decisão");
        let notional = d.quantity * d.entry_price;
        assert!((notional - 200.0).abs() < 1e-6, "notional {notional}");
    }

    /// Stop mais distante tem de gerar posição MENOR — mesmo risco em dólar.
    #[test]
    fn wider_stop_yields_smaller_position() {
        let apertado = build_entry("A", &setup(100.0, 98.0, 2.0), 1000.0, 0.01, 2.0, 1e9).unwrap();
        let largo = build_entry("A", &setup(100.0, 90.0, 2.0), 1000.0, 0.01, 2.0, 1e9).unwrap();
        assert!(
            largo.quantity < apertado.quantity,
            "stop largo deveria reduzir a posição"
        );
    }

    /// Sem teto, um stop apertado pede posição maior que o capital: risco de 3%
    /// com stop de 2,4% pediu notional de 124% da conta em produção.
    #[test]
    fn position_is_capped_regardless_of_how_tight_the_stop_is() {
        // Stop a 1% => a fórmula pediria notional de 100x o risco.
        let s = setup(100.0, 99.0, 2.0);
        let d = build_entry("A", &s, 1000.0, 0.01, 2.0, 30.0).expect("decisão");
        let notional = d.quantity * d.entry_price;
        assert!(
            notional <= 30.0 + 1e-9,
            "notional {notional} passou do teto"
        );
    }

    #[test]
    fn entry_carries_stop_and_target() {
        let s = setup(100.0, 95.0, 2.0);
        let d = build_entry("APTUSDT", &s, 1000.0, 0.01, 2.0, 1e9).unwrap();
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
        assert!(build_entry("A", &s, 0.0, 0.01, 2.0, 1e9).is_none());
        let zero_risk = swing::SwingSetup {
            entry: 100.0,
            stop: 100.0,
            target: 110.0,
            risk_pct: 0.0,
        };
        assert!(build_entry("A", &zero_risk, 1000.0, 0.01, 2.0, 1e9).is_none());
    }

    /// Sem série do BTC o filtro macro não existe. Operar assim seria ignorar
    /// a primeira regra do checklist sem nenhum aviso.
    #[test]
    fn no_entries_without_the_btc_series() {
        let mut store = HashMap::new();
        store.insert("APTUSDT".to_string(), vec![]);
        let out = evaluate_symbols(
            &store,
            &[],
            1000.0,
            0.01,
            2.0,
            30.0,
            &SwingParams::default(),
        );
        assert!(out.is_empty());
    }

    /// A chave precisa separar símbolos E velas. Com uma constante, a primeira
    /// entrada bloqueava todas as outras no executor.
    /// O diagnóstico cobre TODOS os símbolos do store, inclusive os que não têm
    /// setup — é justamente para eles que a matriz existe.
    #[test]
    fn diagnostic_covers_every_symbol_except_the_macro_one() {
        let mut store = HashMap::new();
        store.insert("BTCUSDT".to_string(), Vec::new());
        store.insert("APTUSDT".to_string(), Vec::new());
        store.insert("LINKUSDT".to_string(), Vec::new());
        let snap = diagnose_symbols(&store, &[], &SwingParams::default());
        let nomes: Vec<&str> = snap.symbols.iter().map(|s| s.symbol.as_str()).collect();
        assert_eq!(nomes, vec!["APTUSDT", "LINKUSDT"]);
    }

    /// Símbolo com posição aberta é reportado como tal, e não como bloqueio.
    #[test]
    fn diagnostic_flags_open_positions() {
        let mut store = HashMap::new();
        store.insert("BTCUSDT".to_string(), Vec::new());
        store.insert("APTUSDT".to_string(), Vec::new());
        let snap = diagnose_symbols(
            &store,
            &["APTUSDT".to_string()],
            &SwingParams::default(),
        );
        let apt = &snap.symbols[0];
        assert!(apt.has_position);
        assert_eq!(apt.status, "position_open");
    }

    #[test]
    fn idempotency_key_is_unique_per_symbol_and_candle() {
        let b = candle_bucket(1_754_568_000);
        assert_ne!(swing_event_id("AVAXUSDT", b), swing_event_id("LINKUSDT", b));
        assert_ne!(
            swing_event_id("AVAXUSDT", b),
            swing_event_id("AVAXUSDT", b + 1)
        );
    }

    /// Dois instantes dentro da MESMA vela têm de gerar a mesma chave — é o que
    /// impede o avaliador de 60s republicar o setup 240 vezes.
    #[test]
    fn same_candle_yields_the_same_key() {
        let start = 1_754_568_000u64 / CANDLE_SECS * CANDLE_SECS;
        assert_eq!(candle_bucket(start), candle_bucket(start + CANDLE_SECS - 1));
        assert_ne!(candle_bucket(start), candle_bucket(start + CANDLE_SECS));
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
            30.0,
            &SwingParams::default(),
        );
        assert!(out.is_empty());
    }
}
