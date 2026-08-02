//! Shared configuration helpers for all ViperTrade services.
//!
//! Re-exports common types/enums and provides functions for:
//! - Database URL resolution
//! - Bybit API credentials
//! - Trading mode detection
//! - Bybit base URL selection
//! - Trading pairs configuration
//! - Redis URL resolution

use std::env;
use std::fs;

// --- Re-exported type ---

/// Só existem dois modos: simular localmente (Paper) ou operar valendo
/// (Mainnet). O testnet da Bybit foi removido — o book de lá não reproduz
/// liquidez nem spread reais, então validar estratégia contra ele dava
/// confiança falsa; o Paper já cobre a simulação, com custo e slippage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradingMode {
    Paper,
    Mainnet,
}

// --- TradingMode helpers ---

impl TradingMode {
    /// Parse TRADING_MODE env var. Default: "paper".
    /// Accepted values (case-insensitive):
    /// - "mainnet", "live" → Mainnet
    /// - anything else → Paper
    pub fn from_env() -> Self {
        match env::var("TRADING_MODE")
            .unwrap_or_else(|_| "paper".to_string())
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "mainnet" | "live" => Self::Mainnet,
            _ => Self::Paper,
        }
    }

    /// Get the Bybit REST base URL. Paper reads real market and account data,
    /// so both modes point at production.
    pub fn bybit_base_url(self) -> &'static str {
        "https://api.bybit.com"
    }

    /// Whether this mode uses simulated (database-backed) positions instead of live exchange.
    pub fn uses_simulated_positions(self) -> bool {
        matches!(self, Self::Paper)
    }

    /// Whether this mode executes real exchange orders.
    pub fn executes_exchange_orders(self) -> bool {
        !matches!(self, Self::Paper)
    }

    /// Uppercase label for UI/status (PAPER, MAINNET).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Paper => "PAPER",
            Self::Mainnet => "MAINNET",
        }
    }

    /// Alias for `as_str` used by API.
    pub fn as_status_label(self) -> &'static str {
        self.as_str()
    }

    /// Trade profile label. Only STANDARD remains — SMOKE existed for testnet.
    pub fn trade_profile_label(self) -> &'static str {
        "STANDARD"
    }

    /// Environment label as used by exchange configuration (lowercase).
    pub fn exchange_env_label(self) -> &'static str {
        match self {
            Self::Paper => "paper",
            Self::Mainnet => "mainnet",
        }
    }

    /// Alias for `uses_simulated_positions` used by API.
    pub fn uses_simulated_wallet(self) -> bool {
        self.uses_simulated_positions()
    }
}

/// Resolve Bybit REST base URL.
///
/// `BYBIT_HTTP_PUBLIC` ainda sobrepõe — é o gancho usado para apontar os
/// serviços a um mock em teste. Fora isso é sempre produção: mesmo em Paper os
/// dados de mercado lidos precisam ser os reais.
pub fn resolve_bybit_base_url() -> String {
    read_non_empty_env("BYBIT_HTTP_PUBLIC").unwrap_or_else(|| "https://api.bybit.com".to_string())
}

// --- Environment reading helpers ---

/// Read an env var, trim whitespace, return None if empty or missing.
pub fn read_non_empty_env(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Read a f64 env var with default fallback.
pub fn read_f64_env(name: &str, default: f64) -> f64 {
    env::var(name)
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(default)
}

/// Read a bool env var (accepts "1","true","yes","on" ⇔ "0","false","no","off").
pub fn read_bool_env(name: &str, default: bool) -> bool {
    env::var(name)
        .ok()
        .and_then(|v| match v.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        })
        .unwrap_or(default)
}

/// Read an interval that may be provided in seconds OR minutes (converted to seconds).
/// Tries sec_var first, then min_var (as minutes×60), then returns default_sec.
pub fn read_interval_sec(sec_var: &str, min_var: &str, default_sec: u64) -> u64 {
    if let Some(sec) = env::var(sec_var).ok().and_then(|v| v.parse::<u64>().ok()) {
        return sec;
    }
    if let Some(min) = env::var(min_var).ok().and_then(|v| v.parse::<u64>().ok()) {
        return min.saturating_mul(60);
    }
    default_sec
}

// --- Database URL resolution ---

/// Resolve database connection URL.
///
/// Priority:
/// 1. DATABASE_URL (if non-empty)
/// 2. Individual DB_HOST, DB_PORT (default 5432), DB_NAME, DB_USER, DB_PASSWORD
///
/// Returns `None` if any of DB_HOST/DB_NAME/DB_USER/DB_PASSWORD is missing.
pub fn resolve_database_url() -> Option<String> {
    if let Ok(v) = env::var("DATABASE_URL") {
        if !v.trim().is_empty() {
            return Some(v);
        }
    }

    let host = env::var("DB_HOST").ok()?;
    let port = env::var("DB_PORT")
        .ok()
        .unwrap_or_else(|| "5432".to_string());
    let db = env::var("DB_NAME").ok()?;
    let user = env::var("DB_USER").ok()?;
    let pass = env::var("DB_PASSWORD").ok()?;

    Some(format!(
        "postgresql://{}:{}@{}:{}/{}",
        user, pass, host, port, db
    ))
}

// --- Bybit configuration ---

/// Resolve Bybit API credentials.
///
/// Search order for each credential:
/// 1. BYBIT_MAINNET_API_KEY
/// 2. Unscoped: BYBIT_API_KEY (fallback)
///
/// Returns empty strings if nothing is set (services may allow no-credential modes).
pub fn resolve_bybit_credentials() -> (String, String) {
    // Paper também usa as chaves de mainnet: não envia ordem, mas lê dados
    // reais da conta (taxa efetiva, constraints de símbolo) para simular fielmente.
    let scoped = (
        read_non_empty_env("BYBIT_MAINNET_API_KEY"),
        read_non_empty_env("BYBIT_MAINNET_API_SECRET"),
    );

    (
        scoped
            .0
            .or_else(|| read_non_empty_env("BYBIT_API_KEY"))
            .unwrap_or_default(),
        scoped
            .1
            .or_else(|| read_non_empty_env("BYBIT_API_SECRET"))
            .unwrap_or_default(),
    )
}

// --- Redis URL resolution ---

/// Resolve Redis connection URL. Default: `redis://vipertrade-redis:6379` (K8s internal).
pub fn resolve_redis_url() -> String {
    env::var("REDIS_URL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "redis://vipertrade-redis:6379".to_string())
}

// --- Trading pairs configuration ---

/// Default path to pairs.yaml (K8s container default).
pub fn default_pairs_config_path() -> &'static str {
    "/app/config/pairs.yaml"
}

/// Resolve path to trading pairs configuration file.
/// Priority: STRATEGY_CONFIG env var → default path.
pub fn configured_pairs_path() -> String {
    read_non_empty_env("STRATEGY_CONFIG").unwrap_or_else(|| default_pairs_config_path().to_string())
}

/// Parse the enabled trading pairs from a YAML pairs config.
///
/// Reads the YAML file, iterates top-level keys (symbols), and returns
/// those with `enabled: true`. Skips "global" and "profiles" sections.
/// Result is uppercase-sorted.
pub fn parse_trading_pairs_from_config(path: &str) -> Option<Vec<String>> {
    let raw = fs::read_to_string(path).ok()?;
    let yaml: serde_yaml::Value = serde_yaml::from_str(&raw).ok()?;
    let obj = yaml.as_mapping()?;

    let mut pairs = Vec::new();
    for (key, value) in obj {
        let Some(symbol) = key.as_str() else {
            continue;
        };
        if symbol.eq_ignore_ascii_case("global") || symbol.eq_ignore_ascii_case("profiles") {
            continue;
        }
        let enabled = value
            .as_mapping()
            .and_then(|map| map.get(serde_yaml::Value::from("enabled")))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if enabled {
            pairs.push(symbol.to_uppercase());
        }
    }

    if pairs.is_empty() {
        None
    } else {
        pairs.sort();
        Some(pairs)
    }
}

/// Get configured trading pairs.
///
/// Priority:
/// 1. TRADING_PAIRS comma-separated env var
/// 2. STRATEGY_CONFIG → parse_trading_pairs_from_config()
/// 3. panic with actionable message
pub fn parse_trading_pairs() -> Vec<String> {
    if let Ok(raw) = env::var("TRADING_PAIRS") {
        let parsed: Vec<String> = raw
            .split(',')
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty())
            .collect();

        if !parsed.is_empty() {
            let mut valid = Vec::new();
            let mut rejected = Vec::new();
            for pair in parsed {
                if is_valid_trading_pair(&pair) {
                    valid.push(pair);
                } else {
                    eprintln!("WARN: ignoring invalid trading pair from TRADING_PAIRS: {pair:?}");
                    rejected.push(pair);
                }
            }

            if valid.is_empty() {
                panic!(
                    "TRADING_PAIRS was set but all pairs are invalid (rejected: {}); \
                     pairs must end with USDT, be 7-15 chars, and contain no '/' or spaces",
                    rejected.join(", ")
                );
            }
            return valid;
        }
    }

    let config_path = configured_pairs_path();
    if let Some(pairs) = parse_trading_pairs_from_config(&config_path) {
        return pairs;
    }

    panic!(
        "no trading pairs configured: set TRADING_PAIRS (valid USDT pairs) \
         or provide enabled symbols in {}",
        config_path
    );
}

/// Validate a trading pair symbol.
///
/// Rules: must end with "USDT" (case-insensitive), length 7-15, and contain
/// neither '/' nor whitespace.
pub fn is_valid_trading_pair(pair: &str) -> bool {
    let len = pair.len();
    if !(7..=15).contains(&len) {
        return false;
    }
    if pair.contains('/') || pair.chars().any(|c| c.is_whitespace()) {
        return false;
    }
    pair.to_uppercase().ends_with("USDT")
}

// --- Utility: parse numeric from string ---

/// Parse string to f64, ensuring result is finite.
pub fn parse_f64(raw: &str) -> Option<f64> {
    raw.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Parse string to i64.
pub fn parse_i64(raw: &str) -> Option<i64> {
    raw.parse::<i64>().ok()
}

// Re-export common types used by config consumers
// (TradingMode is defined in this module and thus already public via config::TradingMode)
