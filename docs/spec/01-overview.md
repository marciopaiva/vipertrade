# 01 - Overview

Source: `docs/legacy/VIPERTRADE_SPEC.md` (sections 1.1-1.4).

## Objetivo

ViperTrade e um Lead Trader Bot para Bybit Copy Trading Classic.
Executa estrategia propria e permite copy via Smart Copy Mode.

## Diferenciais

- Engine Tupa para decisoes deterministicas e auditaveis.
- Trailing stop dinamico progressivo.
- Otimizacao para Smart Copy em followers pequenos e medios.
- Perfis de risco (Conservative, Medium, Aggressive).
- Risk management em camadas.

## Metas de Performance Publicas

- Win Rate alvo: 50-60%
- Max Drawdown alvo: <15%
- Profit Factor alvo: >1.5
- Copy Success Rate alvo: >95%

## Capital e Sizing

- Faixa de validacao: `mainnet_micro` com 100 USDT.
- Faixa de producao: 500+ USDT.
- Position sizing alvo por trade: 10-20 USDT.
- Teto de operacao por trade: 30 USDT.

## Referencia Original

- `docs/legacy/VIPERTRADE_SPEC.md`, approximate lines 42-85.
