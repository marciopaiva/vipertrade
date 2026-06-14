# 07 - Configuration

This document describes the main runtime and strategy configuration surfaces used by ViperTrade.

## Configuration layers

ViperTrade is configured through two main layers:

1. `compose/.env` (or `k8s/kind/configmap.yaml` + `secret.yaml` for Kind)
   - environment and infrastructure settings (trading mode, DB, Redis, exchange credentials)
2. `config/trading/pairs.yaml`
   - strategy, risk, and per-token behavior — the single source of truth for all tunables

`config/trading/pairs.yaml` is baked into the service image at build time. Config changes
require: edit the YAML → commit → `make build && make deploy` → `kubectl rollout restart`
the affected deployments. There is no hot-reload and no database config layer.

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

Key fields read by the strategy from `global.mode_profiles.PAPER`:

Entry filters:

- `min_adx` — minimum ADX to allow entry
- `min_trend_score_long` / `min_trend_score_short`
- `min_percent_b_long` / `min_percent_b_short` (Bollinger %B guard)
- `rsi_long_min` / `rsi_long_max` / `rsi_short_min` / `rsi_short_max`
- `max_spread_pct` / `max_atr_pct` / `max_funding_rate_pct`
- `min_volume_24h_usdt` / `min_volume_ratio_long` / `min_volume_ratio_short`
- `min_signal_confirmation_ticks_long` / `min_signal_confirmation_ticks_short`
- `permissive_entry` / `require_multi_exchange_consensus`

Exit controls:

- `stop_loss_pct` / `trailing_enabled` / `min_hold_seconds`
- `stop_loss_cooldown_minutes_long` / `stop_loss_cooldown_minutes_short`
- `opposite_side_exit` — `both` (require consensus AND regime flip), `any`, or `off`
- `thesis_health.*` — thesis-invalidation health thresholds (all disabled: `long_*` = -200, `short_*` = 200)

Sizing (under `global.mode_profiles.PAPER.risk`):

- `max_position_wallet_pct` / `atr_multiplier` / `max_position_usdt`

BTC macro filters:

- `btc_macro_min_trend_score_long` / `btc_macro_min_trend_score_short`
- `btc_macro_min_consensus_count_long` / `btc_macro_min_consensus_count_short`
- `btc_macro_neutral_penalty`

These settings should be treated as high-impact controls.

## Token-level configuration

Each token block in `pairs.yaml` defines:

- `enabled` — whether market-data streams and strategy evaluates this symbol (required)

And optionally, up to three sizing overrides (everything else is global):

- `mode_profiles.<MODE>.risk.max_position_wallet_pct` — wallet cap override
- `risk.atr_multiplier` — ATR multiplier override
- `risk.max_position_usdt` — USDT position cap override

All entry, exit, thesis, and trailing parameters are read from `global.mode_profiles.PAPER`
only. Adding per-symbol `entry_filters` or `trailing_stop` blocks has no effect and
creates a silent shadowing hazard — do not add them.

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

1. Identify a hypothesis from `/analyze/recent` or direct observation.
2. Validate the change with a deterministic sweep before committing:

   ```bash
   kubectl exec deploy/web -- curl -s -X POST http://ai-analyst:8087/sweep \
     -H 'content-type: application/json' \
     -d '{"variants":[{"overrides":[{"path":"mode_profiles.PAPER.<param>","value":<v>}]}]}'
   ```

3. Test on at least two corpus windows (e.g. `limit=60000` and `limit=20000`).
4. If the change is robust, edit `config/trading/pairs.yaml` (under `global.mode_profiles.PAPER`).
5. Commit → `make build && make deploy` → `kubectl rollout restart deployment strategy`.
6. After a wipe or significant change, validate behavior in paper before touching testnet.

Avoid changing multiple unrelated tunables at once — the interactions are hard to attribute.
To reset paper data and start fresh: `make wipe [CONFIRM=yes]`.

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
