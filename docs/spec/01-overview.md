# 01 - Overview

## Objective

ViperTrade is a Lead Trader Bot for Bybit Copy Trading Classic.
It executes proprietary strategy and enables copy via Smart Copy Mode.

## Differentiators

- Tupa engine for deterministic and auditable decisions — the `ViperSmartCopy` pipeline is
  compiled in-process via the `pipeline!` macro (no `.tp` file at runtime).
- Dynamic progressive trailing stop.
- Optimization for Smart Copy with small and medium followers.
- Single-file config source of truth (`pairs.yaml`, gitignored; public template
  `pairs.example.yaml`) with deterministic tuning via backtest sweep.
- Layered risk management.

All eight services are roles of one unified `viper` binary (selected by `VIPER_ROLE`).
Service-to-service transport is Redis **pub/sub** (`viper:market_data`, `viper:decisions`).

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
| ai-analyst | Heuristic diagnostics and deterministic backtest sweep (`/sweep`, `/analyze/recent`) |
| api | REST endpoints |
| web | Operator dashboard |
| postgres | Persistent state (trades, positions, events, audit) |
| redis | Event transport (pub/sub) |

## Runtime Modes

| Mode | Behavior |
|------|----------|
| paper | Mainnet prices, simulated wallet/positions in DB |
| testnet | Bybit testnet with real orders |
| mainnet | Bybit mainnet with real orders |
