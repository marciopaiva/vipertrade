# 01 - Overview

Source: `docs/legacy/VIPERTRADE_SPEC.md` (sections 1.1-1.4).

## Objective

ViperTrade is a Lead Trader Bot for Bybit Copy Trading Classic.
It executes proprietary strategy and enables copy via Smart Copy Mode.

## Differentiators

- Tupa engine for deterministic and auditable decisions.
- Dynamic progressive trailing stop.
- Optimization for Smart Copy with small and medium followers.
- Risk profiles (Conservative, Medium, Aggressive).
- Layered risk management.

## Public Performance Targets

- Win Rate target: 50-60%
- Max Drawdown target: <15%
- Profit Factor target: >1.5
- Copy Success Rate target: >95%

## Capital and Sizing

- Validation range: `mainnet_micro` with 100 USDT.
- Production range: 500+ USDT.
- Position sizing target per trade: 10-20 USDT.
- Maximum operation per trade: 30 USDT.

## Services

| Service | Purpose |
|---------|---------|
| market-data | Exchange signal ingestion |
| strategy | Tupa-driven decision generation |
| executor | Paper/testnet/mainnet execution |
| monitor | Health, reconciliation, drift checks |
| analytics | Market analysis insights |
| ai-analyst | Optional LLM-powered analysis |
| backtest | Historical validation |
| api | REST endpoints |
| web | Operator dashboard |
| postgres | Persistent state |
| redis | Event transport |

## Runtime Modes

| Mode | Behavior |
|------|----------|
| paper | Mainnet prices, simulated wallet/positions in DB |
| testnet | Bybit testnet with real orders |
| mainnet | Bybit mainnet with real orders |

## Reference Original

- `docs/legacy/VIPERTRADE_SPEC.md`, approximate lines 42-85.
