//! Coleta de velas de 4H para a estratégia de swing.
//!
//! Separado do fluxo principal por três razões:
//!
//! - **Cadência.** Uma vela de 4H fecha a cada 4 horas; consultar a cada poucos
//!   segundos, como o fluxo de 1min, seria desperdício e ruído.
//! - **Exchange única.** Usa a Bybit, onde as ordens são executadas. O consenso
//!   de três exchanges existe para filtrar ruído de tick — em 4H os preços já
//!   convergiram, e decidir com o livro onde se opera é mais correto.
//! - **Sem indicadores.** Publica a série crua. Quem calcula EMA e fundo
//!   estrutural é o strategy; se o market-data publicasse indicadores, mudar um
//!   parâmetro da estratégia exigiria mexer no serviço de coleta.

use viper_domain::{stream_publish, OhlcCandle, SwingCandlesEvent, REDIS_STREAM_SWING_CANDLES};

/// Velas necessárias para a EMA de 200 mais folga para o fundo estrutural.
pub(crate) const SWING_CANDLE_LIMIT: usize = 300;
pub(crate) const SWING_INTERVAL: &str = "240"; // minutos, formato da Bybit

/// Converte a resposta de kline da Bybit em velas OHLC.
///
/// A Bybit devolve `[start, open, high, low, close, volume, turnover]` como
/// strings, em ordem decrescente de tempo. A saída sai **crescente**, que é como
/// os cálculos de média esperam a série.
pub(crate) fn parse_ohlc(rows: Vec<Vec<String>>) -> Vec<OhlcCandle> {
    let mut out: Vec<OhlcCandle> = rows
        .into_iter()
        .filter_map(|row| {
            if row.len() < 6 {
                return None;
            }
            let open_time_ms = row[0].parse::<i64>().ok()?;
            let open = row[1].parse::<f64>().ok()?;
            let high = row[2].parse::<f64>().ok()?;
            let low = row[3].parse::<f64>().ok()?;
            let close = row[4].parse::<f64>().ok()?;
            let volume = row[5].parse::<f64>().unwrap_or(0.0).max(0.0);
            // Vela sem preço válido é dado corrompido, não vela de mercado
            // parado — descartar é melhor que propagar zero para uma média.
            if !(close > 0.0 && open > 0.0 && high > 0.0 && low > 0.0) {
                return None;
            }
            if high < low {
                return None;
            }
            Some(OhlcCandle {
                open_time_ms,
                open,
                high,
                low,
                close,
                volume,
            })
        })
        .collect();
    out.sort_by_key(|c| c.open_time_ms);
    out.dedup_by_key(|c| c.open_time_ms);
    out
}

/// Intervalo entre coletas. Uma vela de 4H fecha a cada 4 horas; 5 minutos dá
/// granularidade de sobra para pegar o fechamento sem martelar a API.
pub(crate) const SWING_POLL_SECS: u64 = 300;

/// O BTC entra na coleta mesmo fora do universo operado.
///
/// A primeira regra do checklist é não comprar altcoin contra o Bitcoin caindo.
/// Sem as velas dele o filtro macro não existe — e ele está `enabled: false`
/// justamente porque não se opera BTC com 2x de alavancagem.
pub(crate) const MACRO_SYMBOL: &str = "BTCUSDT";

/// Busca as velas de 4H e publica no stream do swing.
///
/// Falha de um símbolo não interrompe os demais: a coleta é independente por
/// símbolo, e perder um não invalida os outros.
pub(crate) async fn collect_and_publish(
    http: &reqwest::Client,
    conn: &mut redis::aio::MultiplexedConnection,
    base_url: &str,
    symbols: &[String],
) {
    // O BTC precisa ser coletado mesmo não sendo operado: é o filtro macro.
    let mut targets: Vec<String> = symbols.to_vec();
    if !targets.iter().any(|s| s == MACRO_SYMBOL) {
        targets.push(MACRO_SYMBOL.to_string());
    }

    for symbol in &targets {
        let url = swing_kline_url(base_url, symbol);
        let rows = match fetch_kline_rows(http, &url).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(symbol = %symbol, error = %e, "Failed to fetch 4H candles");
                continue;
            }
        };
        let candles = parse_ohlc(rows);
        if candles.len() < 200 {
            tracing::warn!(
                symbol = %symbol, got = candles.len(),
                "Not enough 4H candles for the slow average — skipping"
            );
            continue;
        }
        let event = SwingCandlesEvent::new(symbol.clone(), "4h".to_string(), candles);
        if let Err(e) = event.validate() {
            tracing::warn!(symbol = %symbol, error = %e, "Invalid swing candles event dropped");
            continue;
        }
        match serde_json::to_string(&event) {
            Ok(json) => {
                if let Err(e) = stream_publish(conn, REDIS_STREAM_SWING_CANDLES, &json).await {
                    tracing::warn!(symbol = %symbol, error = %e, "Failed to publish swing candles");
                } else {
                    tracing::info!(
                        symbol = %symbol, candles = event.candles.len(),
                        "Published 4H candles"
                    );
                }
            }
            Err(e) => {
                tracing::warn!(symbol = %symbol, error = %e, "Failed to serialize swing candles")
            }
        }
    }
}

/// Extrai `result.list` da resposta de kline da Bybit.
async fn fetch_kline_rows(http: &reqwest::Client, url: &str) -> Result<Vec<Vec<String>>, String> {
    let resp = http.get(url).send().await.map_err(|e| e.to_string())?;
    let value: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let ret_code = value.get("retCode").and_then(|v| v.as_i64()).unwrap_or(-1);
    if ret_code != 0 {
        return Err(format!("retCode={ret_code}"));
    }
    let list = value
        .get("result")
        .and_then(|r| r.get("list"))
        .and_then(|l| l.as_array())
        .ok_or_else(|| "missing result.list".to_string())?;
    Ok(list
        .iter()
        .filter_map(|row| {
            row.as_array().map(|cells| {
                cells
                    .iter()
                    .map(|c| c.as_str().unwrap_or_default().to_string())
                    .collect()
            })
        })
        .collect())
}

/// URL de kline de 4H da Bybit para o símbolo.
pub(crate) fn swing_kline_url(base_url: &str, symbol: &str) -> String {
    format!(
        "{}/v5/market/kline?category=linear&symbol={}&interval={}&limit={}",
        base_url, symbol, SWING_INTERVAL, SWING_CANDLE_LIMIT
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(ts: &str, o: &str, h: &str, l: &str, c: &str) -> Vec<String> {
        vec![
            ts.into(),
            o.into(),
            h.into(),
            l.into(),
            c.into(),
            "100".into(),
            "0".into(),
        ]
    }

    /// A Bybit devolve do mais recente para o mais antigo; médias e fundos
    /// dependem da ordem cronológica. Inverter isso silenciosamente daria
    /// EMA calculada ao contrário.
    #[test]
    fn parse_returns_candles_in_chronological_order() {
        let rows = vec![
            row("3000", "12", "13", "11", "12.5"),
            row("1000", "10", "11", "9", "10.5"),
            row("2000", "11", "12", "10", "11.5"),
        ];
        let out = parse_ohlc(rows);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].open_time_ms, 1000);
        assert_eq!(out[2].open_time_ms, 3000);
    }

    #[test]
    fn parse_keeps_open_which_the_candlestick_patterns_need() {
        let out = parse_ohlc(vec![row("1000", "10", "13", "9", "12")]);
        assert_eq!(out[0].open, 10.0);
        assert_eq!(out[0].high, 13.0);
        assert_eq!(out[0].low, 9.0);
        assert_eq!(out[0].close, 12.0);
    }

    #[test]
    fn parse_drops_corrupt_rows() {
        let rows = vec![
            row("1000", "10", "11", "9", "10.5"),
            row("2000", "0", "11", "9", "10.5"), // open zerado
            row("3000", "10", "11", "9", "0"),   // close zerado
            row("4000", "10", "8", "9", "10.5"), // high < low
            vec!["5000".into(), "10".into()],    // colunas faltando
        ];
        let out = parse_ohlc(rows);
        assert_eq!(out.len(), 1, "só a primeira linha é válida");
        assert_eq!(out[0].open_time_ms, 1000);
    }

    /// Repetição de timestamp acontece quando a vela corrente é republicada
    /// entre consultas; duplicar distorce média e contagem.
    #[test]
    fn parse_deduplicates_by_timestamp() {
        let rows = vec![
            row("1000", "10", "11", "9", "10.5"),
            row("1000", "10", "11", "9", "10.7"),
            row("2000", "11", "12", "10", "11.5"),
        ];
        assert_eq!(parse_ohlc(rows).len(), 2);
    }

    #[test]
    fn kline_url_requests_four_hour_candles() {
        let u = swing_kline_url("https://api.bybit.com", "APTUSDT");
        assert!(u.contains("interval=240"), "4H = 240 minutos: {u}");
        assert!(u.contains("symbol=APTUSDT"));
        assert!(u.contains(&format!("limit={SWING_CANDLE_LIMIT}")));
    }

    /// A EMA de 200 precisa de pelo menos 200 velas; pedir menos deixaria a
    /// estratégia sem tendência macro e sem nenhum erro aparente.
    /// Sem as velas do BTC não há filtro macro, e a regra número um do
    /// checklist deixa de existir sem nenhum erro aparente.
    #[test]
    fn macro_symbol_is_bitcoin() {
        assert_eq!(MACRO_SYMBOL, "BTCUSDT");
    }

    #[test]
    fn candle_limit_covers_the_slow_average() {
        assert!(
            SWING_CANDLE_LIMIT >= 200,
            "limite {SWING_CANDLE_LIMIT} não cobre a EMA200"
        );
    }
}
