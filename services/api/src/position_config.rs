use serde::Deserialize;
use std::collections::HashMap;

#[derive(Clone)]
pub struct GlobalPositionConfig {
    pub trailing_enabled: bool,
    pub _trailing_min_move_threshold_pct: f64,
}

impl Default for GlobalPositionConfig {
    fn default() -> Self {
        Self {
            trailing_enabled: true,
            _trailing_min_move_threshold_pct: 0.002,
        }
    }
}

#[derive(Clone)]
pub struct PairPositionConfig {
    pub stop_loss_pct: f64,
    pub take_profit_pct: f64,
    pub trailing_by_profile: HashMap<String, TrailingProfileConfig>,
    pub trailing_enabled: Option<bool>,
}

#[derive(Clone)]
pub struct TrailingProfileConfig {
    pub activate_after_profit_pct: f64,
    pub move_to_break_even_at: f64,
}

#[derive(Clone, Default)]
pub struct ModePositionConfig {
    pub stop_loss_pct: Option<f64>,
    pub take_profit_pct: Option<f64>,
    pub trailing_enabled: Option<bool>,
    pub fixed_take_profit_enabled: Option<bool>,
    pub trailing: Option<TrailingProfileConfig>,
}

#[derive(Clone, Default)]
pub struct PositionConfigStore {
    pub global: GlobalPositionConfig,
    pub pairs: HashMap<String, PairPositionConfig>,
    pub mode_profiles: HashMap<String, ModePositionConfig>,
}

#[derive(Debug, Deserialize)]
struct PairsFile {
    global: Option<PairsGlobalSection>,
    #[serde(flatten)]
    pairs: HashMap<String, PairFileSection>,
}

#[derive(Debug, Deserialize)]
struct PairsGlobalSection {
    mode_profiles: Option<HashMap<String, ModeProfileSection>>,
    trailing_stop: Option<GlobalTrailingSection>,
}

#[derive(Debug, Deserialize)]
struct ModeProfileSection {
    stop_loss_pct: Option<f64>,
    take_profit_pct: Option<f64>,
    trailing_enabled: Option<bool>,
    fixed_take_profit_enabled: Option<bool>,
    trailing_stop: Option<ModeTrailingSection>,
}

#[derive(Debug, Deserialize)]
struct ModeTrailingSection {
    activate_after_profit_pct: Option<f64>,
    move_to_break_even_at: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct GlobalTrailingSection {
    enabled: Option<bool>,
    min_move_threshold_pct: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct PairFileSection {
    risk: Option<PairRiskSection>,
    trailing_stop: Option<PairTrailingSection>,
}

#[derive(Debug, Deserialize)]
struct PairRiskSection {
    stop_loss_pct: Option<f64>,
    take_profit_pct: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct PairTrailingSection {
    enabled: Option<bool>,
    by_profile: Option<HashMap<String, PairTrailingProfileSection>>,
}

#[derive(Debug, Deserialize)]
struct PairTrailingProfileSection {
    activate_after_profit_pct: Option<f64>,
    move_to_break_even_at: Option<f64>,
}

pub fn load_position_config(path: &str) -> PositionConfigStore {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(err) => {
            tracing::warn!(path = %path, error = %err, "Failed to read position config");
            return PositionConfigStore::default();
        }
    };

    let parsed: PairsFile = match serde_yaml::from_str(&raw) {
        Ok(parsed) => parsed,
        Err(err) => {
            tracing::warn!(path = %path, error = %err, "Failed to parse position config");
            return PositionConfigStore::default();
        }
    };

    let global = GlobalPositionConfig {
        trailing_enabled: parsed
            .global
            .as_ref()
            .and_then(|g| g.trailing_stop.as_ref())
            .and_then(|t| t.enabled)
            .unwrap_or(true),
        _trailing_min_move_threshold_pct: parsed
            .global
            .as_ref()
            .and_then(|g| g.trailing_stop.as_ref())
            .and_then(|t| t.min_move_threshold_pct)
            .unwrap_or(0.002),
    };

    let mode_profiles = parsed
        .global
        .as_ref()
        .and_then(|g| g.mode_profiles.as_ref())
        .map(|profiles| {
            profiles
                .iter()
                .map(|(mode, cfg)| {
                    (
                        mode.to_uppercase(),
                        ModePositionConfig {
                            stop_loss_pct: cfg.stop_loss_pct,
                            take_profit_pct: cfg.take_profit_pct,
                            trailing_enabled: cfg.trailing_enabled,
                            fixed_take_profit_enabled: cfg.fixed_take_profit_enabled,
                            trailing: cfg.trailing_stop.as_ref().map(|ts| TrailingProfileConfig {
                                activate_after_profit_pct: ts
                                    .activate_after_profit_pct
                                    .unwrap_or(0.015),
                                move_to_break_even_at: ts.move_to_break_even_at.unwrap_or(0.02),
                            }),
                        },
                    )
                })
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();

    let mut pairs = HashMap::new();
    for (symbol, pair) in parsed.pairs {
        let Some(risk) = pair.risk else {
            continue;
        };
        let stop_loss_pct = risk.stop_loss_pct.unwrap_or(0.015);
        let take_profit_pct = risk.take_profit_pct.unwrap_or(0.03);
        let trailing_enabled = pair.trailing_stop.as_ref().and_then(|t| t.enabled);
        let trailing_by_profile = pair
            .trailing_stop
            .and_then(|t| t.by_profile)
            .unwrap_or_default()
            .into_iter()
            .map(|(profile, cfg)| {
                (
                    profile.to_uppercase(),
                    TrailingProfileConfig {
                        activate_after_profit_pct: cfg.activate_after_profit_pct.unwrap_or(0.015),
                        move_to_break_even_at: cfg.move_to_break_even_at.unwrap_or(0.02),
                    },
                )
            })
            .collect::<HashMap<_, _>>();

        pairs.insert(
            symbol.to_uppercase(),
            PairPositionConfig {
                stop_loss_pct,
                take_profit_pct,
                trailing_by_profile,
                trailing_enabled,
            },
        );
    }

    PositionConfigStore {
        global,
        pairs,
        mode_profiles,
    }
}

pub fn default_trailing_profile() -> TrailingProfileConfig {
    TrailingProfileConfig {
        activate_after_profit_pct: 0.015,
        move_to_break_even_at: 0.02,
    }
}