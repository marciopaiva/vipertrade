# 03 - Risk and Profiles

Source: `docs/legacy/VIPERTRADE_SPEC.md` (section 4).

## Profiles

| Profile | Risk/Trade | Max Leverage | Max Daily Loss |
|---------|------------|--------------|----------------|
| Conservative | 0.75% | 2x | 2% |
| Medium | 1.25% | 2x | 3% |
| Aggressive | 2.00% | 3x | 5% |

Profiles are configured in `config/trading/pairs.yaml` under `profiles:`.

## Main Parameters

- Risk per trade (percentage of capital)
- Stop loss / take profit percentages
- Max leverage (1-3x depending on profile)
- Max daily loss (circuit breaker)
- Max open positions (4 default for PAPER mode)
- Max position USDT (5-30 via Smart Copy)
- Max total exposure cap

## Circuit Breakers

- Consecutive loss limit triggers cooldown.
- Daily loss limit pauses new entries until reset.
- Kill switch blocks all new entries immediately.
- High drift alerts pause entries via `allow_long`/`allow_short` per symbol.

## Operational Rules

- Capital preservation takes priority over growth.
- Profile adjustments must be auditable and version-controlled.
- Changes to risk parameters should be validated in paper mode first.

## Reference Original

- `docs/legacy/VIPERTRADE_SPEC.md`, section 4.
