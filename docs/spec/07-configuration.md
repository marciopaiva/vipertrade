# 07 - Configuration

This document describes the main runtime and strategy configuration surfaces used by ViperTrade.

## Configuration layers

ViperTrade is configured through three main layers:

1. `compose/.env`
   - environment and infrastructure settings
2. `config/trading/pairs.yaml`
   - strategy, risk, and per-token behavior
3. service defaults in Rust
   - fallback behavior used when configuration is omitted

Safe operation depends on understanding how these layers interact.

## Runtime mode

The most important runtime switch is `TRADING_MODE`:

- `paper`
  - mainnet prices with simulated positions and wallet in PostgreSQL
- `testnet`
  - real execution path on Bybit testnet
- `mainnet`
  - real execution path on Bybit mainnet

Recommended rollout order:

1. `paper`
2. `testnet`
3. `mainnet`

## Key environment variables

Common environment variables in `compose/.env`:

- `TRADING_MODE`
  - selects `paper`, `testnet`, or `mainnet`
- `RUST_BUILDER_IMAGE`
- `RUST_RUNTIME_IMAGE`
- `STRATEGY_BUILDER_IMAGE`
- `STRATEGY_RUNTIME_IMAGE`
- `WEB_BASE_IMAGE`
  - image tags used by the build and compose scripts
- `OPERATOR_API_TOKEN`
  - required for authenticated control actions such as kill switch
- `BYBIT_*`
  - exchange credentials and exchange environment settings

## `pairs.yaml` structure

`config/trading/pairs.yaml` contains:

- `global`
  - shared strategy and risk defaults
- `global.mode_profiles`
  - mode-specific overlays for `PAPER`, `TESTNET`, and `MAINNET`
- token-level blocks such as `DOGEUSDT`, `XRPUSDT`, `ADAUSDT`
  - per-token overrides

## Mode profiles

Mode profiles define the broad risk posture of the runtime.

Common fields used there include:

- `permissive_entry`
- `require_multi_exchange_consensus`
- `require_btc_macro_alignment`
- `prefer_bybit_for_decisions`
- `min_volume_24h_usdt`
- `max_spread_pct`
- `max_atr_pct`
- `max_funding_rate_pct`
- `stop_loss_pct`
- `trailing_enabled`
- `min_hold_seconds`
- `min_trend_score_long`
- `min_trend_score_short`
- `min_signal_confirmation_ticks_long`
- `min_signal_confirmation_ticks_short`
- `btc_macro_min_trend_score_long`
- `btc_macro_min_trend_score_short`
- `btc_macro_min_consensus_count_long`
- `btc_macro_min_consensus_count_short`
- `btc_macro_neutral_penalty`
- `min_volume_ratio_long`
- `min_volume_ratio_short`
- `rsi_long_min`
- `rsi_long_max`
- `rsi_short_min`
- `rsi_short_max`
- `stop_loss_cooldown_minutes_long`
- `stop_loss_cooldown_minutes_short`

These settings should be treated as high-impact controls.

## Token-level configuration

Each token block may define:

- `enabled`
- `category`
- `volatility_profile`
- `mode_profiles`
- `risk`
- `trailing_stop`
- `entry_filters`

Common token-level `entry_filters` and risk controls include:

- `min_trend_score_long`
- `min_trend_score_short`
- `min_signal_confirmation_ticks`
- `min_signal_confirmation_ticks_long`
- `min_signal_confirmation_ticks_short`
- `thesis_invalidation_confirmation_ticks`
- `stop_loss_cooldown_minutes_long`
- `stop_loss_cooldown_minutes_short`
- `max_atr_pct`
- `max_position_usdt`

This is where token-specific behavior is tuned.

Examples:

- tighter `max_position_usdt` for more volatile tokens
- higher `min_trend_score_short` for unstable short setups
- additional confirmation ticks for noisy pairs
- earlier thesis invalidation for weak-follow-through symbols

## High-risk tuning areas

The following settings can materially change runtime behavior and should be changed carefully:

- token universe selection
- per-token `enabled` state
- `max_position_usdt`
- `max_funding_rate_pct`
- `stop_loss_pct`
- trailing stop activation and ratchet levels
- `min_trend_score_long`
- `min_trend_score_short`
- `prefer_bybit_for_decisions`
- `min_signal_confirmation_ticks_long`
- `min_signal_confirmation_ticks_short`
- BTC macro filters
- stop-loss cooldown behavior
- thesis invalidation confirmation behavior

## Safe tuning workflow

Recommended workflow for configuration changes:

1. change one class of variables at a time
2. validate locally with:
   - `make validate-ci`
   - `make validate-runtime`
3. run in `paper`
4. collect enough closed trades to evaluate behavior
5. only then promote to `testnet`

Avoid changing:

- token universe
- trend thresholds
- confirmation ticks
- trailing behavior

all at once.

## Operational guidance

When tuning tokens, treat each pair as its own behavior profile.

Do not assume that settings that work for:

- `ADAUSDT`

will also work for:

- `DOGEUSDT`
- `SUIUSDT`
- `NEARUSDT`

Different token sets and threshold combinations can change both trade frequency and loss profile significantly.

## Recommendation

Use conservative defaults, small rollout steps, and evidence-based tuning.

ViperTrade is configurable enough to be powerful, but that also means configuration discipline is part of operating it safely.
