# 03 - Risk and Profiles

## Overview

All risk parameters live in `config/trading/pairs.yaml`. There are no separate profile
files and no profile switching at runtime — the `TRADING_MODE` env selects the execution
surface (paper/testnet/mainnet) and the `TRADING_PROFILE` env is a label only.

`global.mode_profiles.PAPER` is the sole source of truth for all strategy and risk
tunables. `MAINNET` inherits from the same anchor (`*paper_profile`) — promotion to
mainnet is an explicit human action (edit + commit + deploy), not a runtime flag.

`pairs.yaml` is **gitignored** (private tuning). The repository ships a sanitized
template, `config/trading/pairs.example.yaml`; `scripts/init-secrets.sh` seeds the real
file from it on first run. The values shown below are the current PAPER defaults.

## Risk Parameters in Effect

**Position sizing** (`global.risk`):

- `risk_per_trade_pct: 1.25` — risk budget per trade as % of equity
- `max_leverage: 2` — maximum leverage cap
- `max_daily_loss_pct: 0.03` — daily loss circuit breaker (3%)
- `max_consecutive_losses: 3` — consecutive-loss circuit breaker

**Sizing defaults** (`global.mode_profiles.PAPER.risk`):

- `max_position_wallet_pct: 0.08` — max position as % of wallet
- `atr_multiplier: 0.65` — ATR multiplier for smart sizing
- `max_position_usdt: 18` — max position size in USDT

These defaults apply to every symbol. Per-symbol blocks may override the three sizing
keys above; all other tunables are global-only.

## Per-Symbol Overrides

Each token block in `pairs.yaml` may contain:

- `enabled` — whether market-data streams and strategy evaluates this symbol
- `mode_profiles.<MODE>.risk.max_position_wallet_pct` — per-symbol wallet cap
- `risk.atr_multiplier` — per-symbol ATR multiplier
- `risk.max_position_usdt` — per-symbol USDT cap

All entry, exit, thesis, and trailing parameters are global (read from
`global.mode_profiles.PAPER` by `mode_f64`/`mode_i64`). Per-symbol entry_filters and
trailing_stop blocks have no readers and must not be added to avoid silent shadowing.

## Circuit Breakers

- Consecutive loss limit triggers a cooldown (no new entries).
- Daily loss limit pauses new entries until reset.
- Kill switch (`/api/v1/control/kill-switch`) blocks all new entries immediately.
- High drift alerts pause entries via `allow_long`/`allow_short` per symbol (monitor service).

## Operational Rules

- Capital preservation takes priority over growth.
- Tune in paper mode first; validate with the deterministic `/sweep` before committing.
- Config changes require edit → commit → `make build && make deploy` → rollout restart.
- Use `make wipe` to reset paper data and restart services with a clean slate.
