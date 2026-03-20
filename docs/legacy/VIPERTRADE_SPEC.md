# 🐍 **VIPERTRADE v0.8.0 - Especificação Técnica Completa**

> **Lead Trader Bot para Bybit Copy Trading Classic**  
> *Otimizado para Smart Copy Mode com Trailing Stop Dinâmico*  
> *Integrado com Tupã v0.8.0 (crates.io)*

**Versão:** 0.8.0  
**Última Atualização:** Março 2026  
**Status:** ✅ 100% Especificado - Pronto para Implementação  
**Pares de Trading:** DOGEUSDT, XRPUSDT, TRXUSDT, XLMUSDT  
**Modo de Copy:** Smart Copy (Recomendado)  
**Trailing Stop:** Dinâmico Progressivo (Ratcheting)  
**Tupã Integration:** crates.io v0.8.0 (stable)

---

## 📋 **Índice**

```
1. Visão Geral do Projeto
2. Arquitetura do Sistema
3. Configuração dos 4 Pares de Trading
4. Risk Management e Perfis de Trading
5. Database Schema
6. Bybit API Integration
7. Error Handling Matrix
8. WebSocket Reconnection Strategy
9. Disaster Recovery Procedures
10. Secrets Management
11. Discord Notifications
12. Tupã Language Integration (crates.io v0.8.0)
13. Lead Trader Operations
14. Backtesting Engine
15. Smart Copy Mode Optimization
16. Dynamic Trailing Stop
17. Blocos de Desenvolvimento
18. Checklist de Validação
19. Comandos e Scripts Úteis
20. API Reference
21. Versionamento e Compatibilidade Tupã
22. Tupã crates.io Integration
```

---

## 1️⃣ **Visão Geral do Projeto**

### **1.1 Objetivo**

ViperTrade é um **Lead Trader Bot** automatizado para a plataforma **Bybit Copy Trading Classic**. Ele executa estratégia própria de trading e permite que outros usuários da Bybit copiem suas operações automaticamente via Smart Copy Mode.

### **1.2 Diferenciais Competitivos**

| Diferencial | Descrição | Benefício |
|------------|-----------|-----------|
| **Tupã Engine v0.8.0** | Linguagem de orquestração determinística via crates.io | Auditabilidade completa, builds estáveis, semver |
| **Trailing Stop Dinâmico** | Ajusta progressivamente conforme lucro aumenta (ratcheting) | Protege ganhos, deixa winners correrem |
| **Smart Copy Optimized** | Position sizing previsível ($5-$30) e estável | Menos falhas de copy, mais followers |
| **3 Perfis de Risco** | Conservative / Medium / Aggressive com parâmetros distintos | Adaptável a diferentes condições de mercado |
| **Risk Management Multi-Camada** | 4 níveis de proteção + circuit breakers nativos | Preservação de capital em primeiro lugar |

### **1.3 Metas de Performance (Públicas para Followers)**

| Métrica | Target (30d) | Importância |
|---------|-------------|-------------|
| Win Rate | 50-60% | Alta - atrai followers |
| Max Drawdown | < 15% | Crítica - retém followers |
| Profit Factor | > 1.5 | Alta - credibilidade |
| Avg Risk/Reward | ≥ 2:1 | Média - consistência |
| Trades/Mês | 20-50 | Média - atividade sem overtrading |
| Copy Success Rate | > 95% | Crítica - evita auto-unfollow |

### **1.4 Capital e Sizing**

```yaml
initial_capital:
  testnet: 10000  # USDT fake para testes
  mainnet_micro: 100  # USDT reais para validação
  mainnet_production: 500+  # USDT para escalar

position_sizing:
  min_position_usdt: 5  # Abaixo não copia bem
  max_position_usdt: 30  # Acima exclui followers pequenos
  target_position_usdt: 10-20  # Sweet spot para Smart Copy
  risk_per_trade_pct: 0.75-2.0  # Dependendo do perfil
```

---

## 2️⃣ **Arquitetura do Sistema**

### **2.1 Diagrama de Componentes**

```
┌─────────────────────────────────────────────────────────────────┐
│                    VIPERTRADE v0.8.0                            │
│                    (Lead Trader Bot)                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐     │
│  │ Market Data  │ →  │   Strategy   │ →  │   Executor   │     │
│  │ (Bybit WS)   │    │   (Tupã)     │    │   (Orders)   │     │
│  └──────────────┘    └──────────────┘    └──────────────┘     │
│         │                   │                    │              │
│         ▼                   ▼                    ▼              │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐     │
│  │    Redis     │    │  Risk Mgmt   │    │   Bybit API  │     │
│  │   (Cache)    │    │ + Trailing   │    │ (REST + WS)  │     │
│  └──────────────┘    └──────────────┘    └──────────────┘     │
│                              │                                  │
│                              ▼                                  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                    PostgreSQL                            │  │
│  │  • trades (histórico)                                    │  │
│  │  • position_snapshots (reconciliação)                    │  │
│  │  • system_events (audit trail)                           │  │
│  │  • daily_metrics (agregados)                             │  │
│  │  • tupa_audit_logs (logs estruturados)                   │  │
│  └──────────────────────────────────────────────────────────┘  │
│                              │                                  │
│                              ▼                                  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                  Monitor Service                         │  │
│  │  • Health Checks                                         │  │
│  │  • Discord Notifications                                 │  │
│  │  • Reconciliation Engine                                 │  │
│  │  • Trailing Stop Monitor                                 │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │              BYBIT COPY TRADING CLASSIC                  │  │
│  │  • ViperTrade = Lead Trader                              │  │
│  │  • Smart Copy Mode para followers                        │  │
│  │  • Métricas públicas (win rate, ROI, drawdown)           │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### **2.2 Serviços Docker/Podman**

| Serviço | Função | Memória | CPU | Restart |
|---------|--------|---------|-----|---------|
| postgres | Database | 512MB | 0.5 | unless-stopped |
| redis | Cache/PubSub | 128MB | 0.25 | unless-stopped |
| market-data | WebSocket Subscriber | 256MB | 0.5 | unless-stopped |
| strategy | Tupã Engine + Risk + Trailing | 512MB | 1.0 | unless-stopped |
| executor | Order Execution | 256MB | 0.5 | unless-stopped |
| monitor | Health + Alerts + Reconcile | 256MB | 0.5 | unless-stopped |
| web | Dashboard (opcional) | 512MB | 0.5 | unless-stopped |
| api | API Backend (opcional) | 256MB | 0.5 | unless-stopped |

### **2.3 Fluxo de Decisão de Trading**

```
1. Bybit WebSocket → Market Data → Redis Pub/Sub
2. Redis Pub/Sub → Tupã Strategy Engine → Decision (ENTER_LONG/ENTER_SHORT/HOLD/CLOSE)
3. Decision → Risk Manager → Validate (daily loss, exposure, circuit breakers)
4. Validated → Order Executor → Bybit REST API (com OCO: SL + TP)
5. Execution Result → Database + Monitor
6. Monitor → Trailing Stop (ajusta dinamicamente) + Discord Webhook (se necessário)
7. Reconciliation → Bybit API vs Local DB (a cada 5min)
8. Audit → tupa_audit_logs → JSON estruturado para compliance
```

---

## 3️⃣ **Configuração dos 4 Pares de Trading**

### **3.1 Pares Suportados**

```yaml
trading_pairs:
  - symbol: "DOGEUSDT"
    category: "meme_coin"
    volatility: "high"
    min_24h_volume_usdt: 50000000
    
  - symbol: "XRPUSDT"
    category: "large_cap_alt"
    volatility: "medium-high"
    min_24h_volume_usdt: 100000000
    
  - symbol: "TRXUSDT"
    category: "large_cap_alt"
    volatility: "medium"
    min_24h_volume_usdt: 80000000
    
  - symbol: "XLMUSDT"
    category: "large_cap_alt"
    volatility: "medium-high"
    min_24h_volume_usdt: 60000000
```

### **3.2 Parâmetros Específicos por Par (config/trading/pairs.yaml)**

```yaml
# config/trading/pairs.yaml
# ViperTrade v0.8.0 - Trading Pairs Configuration
# Otimizado para Smart Copy Mode com Trailing Stop Dinâmico

global:
  smart_copy:
    enabled: true
    min_position_usdt: 5
    max_position_usdt: 30
    target_position_usdt: 10-20
    max_position_change_pct: 0.30
  
  trailing_stop:
    enabled: true
    check_interval_ms: 500
    max_updates_per_min: 10
    min_move_threshold_pct: 0.002
  
  risk:
    max_daily_loss_pct: 0.03
    max_consecutive_losses: 3
    max_open_positions: 2
    max_total_exposure_pct: 0.50
  
  entry_filters:
    avoid_hours_utc: ["00:00-02:00", "22:00-23:59"]
    min_volume_24h_usdt: 30000000
    max_spread_pct: 0.001
    max_atr_pct: 0.05
    max_funding_rate_pct: 0.015

DOGEUSDT:
  enabled: true
  category: "meme_coin"
  volatility_profile: "high"
  
  bybit:
    price_precision: 5
    qty_precision: 0
    tick_size: 0.00001
    step_size: 1
    min_order_value_usdt: 5.00
    max_order_value_usdt: 50.00
  
  risk:
    stop_loss_pct: 0.02
    take_profit_pct: 0.04
    max_position_usdt: 15.00
    atr_period: 14
    atr_multiplier: 0.5
  
  trailing_stop:
    enabled: true
    by_profile:
      CONSERVATIVE:
        activate_after_profit_pct: 0.015
        initial_trail_pct: 0.008
        ratchet_levels:
          - at_profit_pct: 0.03
            trail_pct: 0.012
          - at_profit_pct: 0.06
            trail_pct: 0.02
        move_to_break_even_at: 0.02
      MEDIUM:
        activate_after_profit_pct: 0.02
        initial_trail_pct: 0.01
        ratchet_levels:
          - at_profit_pct: 0.04
            trail_pct: 0.015
          - at_profit_pct: 0.08
            trail_pct: 0.025
        move_to_break_even_at: 0.025
      AGGRESSIVE:
        activate_after_profit_pct: 0.025
        initial_trail_pct: 0.012
        ratchet_levels:
          - at_profit_pct: 0.05
            trail_pct: 0.02
          - at_profit_pct: 0.10
            trail_pct: 0.035
        move_to_break_even_at: 0.03
  
  liquidity:
    min_24h_volume_usdt: 50000000
    max_spread_pct: 0.001
    avoid_during_low_liquidity: true
  
  smart_copy:
    recommended_for_beginners: false
    warning: "High volatility - use Conservative profile"

XRPUSDT:
  enabled: true
  category: "large_cap_alt"
  volatility_profile: "medium-high"
  
  bybit:
    price_precision: 4
    qty_precision: 1
    tick_size: 0.0001
    step_size: 1
    min_order_value_usdt: 5.00
    max_order_value_usdt: 50.00
  
  risk:
    stop_loss_pct: 0.018
    take_profit_pct: 0.036
    max_position_usdt: 20.00
    atr_period: 14
    atr_multiplier: 0.6
  
  trailing_stop:
    enabled: true
    by_profile:
      CONSERVATIVE:
        activate_after_profit_pct: 0.012
        initial_trail_pct: 0.006
        ratchet_levels:
          - at_profit_pct: 0.025
            trail_pct: 0.01
          - at_profit_pct: 0.05
            trail_pct: 0.018
        move_to_break_even_at: 0.018
      MEDIUM:
        activate_after_profit_pct: 0.015
        initial_trail_pct: 0.008
        ratchet_levels:
          - at_profit_pct: 0.03
            trail_pct: 0.012
          - at_profit_pct: 0.06
            trail_pct: 0.02
        move_to_break_even_at: 0.02
      AGGRESSIVE:
        activate_after_profit_pct: 0.02
        initial_trail_pct: 0.01
        ratchet_levels:
          - at_profit_pct: 0.04
            trail_pct: 0.018
          - at_profit_pct: 0.08
            trail_pct: 0.03
        move_to_break_even_at: 0.025
  
  liquidity:
    min_24h_volume_usdt: 100000000
    max_spread_pct: 0.001
    avoid_during_low_liquidity: true
  
  smart_copy:
    recommended_for_beginners: true
    warning: null

TRXUSDT:
  enabled: true
  category: "large_cap_alt"
  volatility_profile: "medium"
  
  bybit:
    price_precision: 5
    qty_precision: 0
    tick_size: 0.00001
    step_size: 1
    min_order_value_usdt: 5.00
    max_order_value_usdt: 50.00
  
  risk:
    stop_loss_pct: 0.015
    take_profit_pct: 0.030
    max_position_usdt: 20.00
    atr_period: 14
    atr_multiplier: 0.7
  
  trailing_stop:
    enabled: true
    by_profile:
      CONSERVATIVE:
        activate_after_profit_pct: 0.01
        initial_trail_pct: 0.005
        ratchet_levels:
          - at_profit_pct: 0.02
            trail_pct: 0.008
          - at_profit_pct: 0.04
            trail_pct: 0.015
        move_to_break_even_at: 0.015
      MEDIUM:
        activate_after_profit_pct: 0.015
        initial_trail_pct: 0.008
        ratchet_levels:
          - at_profit_pct: 0.03
            trail_pct: 0.012
          - at_profit_pct: 0.06
            trail_pct: 0.02
        move_to_break_even_at: 0.02
      AGGRESSIVE:
        activate_after_profit_pct: 0.02
        initial_trail_pct: 0.01
        ratchet_levels:
          - at_profit_pct: 0.05
            trail_pct: 0.018
          - at_profit_pct: 0.10
            trail_pct: 0.03
        move_to_break_even_at: 0.025
  
  liquidity:
    min_24h_volume_usdt: 80000000
    max_spread_pct: 0.001
    avoid_during_low_liquidity: true
  
  smart_copy:
    recommended_for_beginners: true
    warning: null

XLMUSDT:
  enabled: true
  category: "large_cap_alt"
  volatility_profile: "medium-high"
  
  bybit:
    price_precision: 5
    qty_precision: 0
    tick_size: 0.00001
    step_size: 1
    min_order_value_usdt: 5.00
    max_order_value_usdt: 50.00
  
  risk:
    stop_loss_pct: 0.018
    take_profit_pct: 0.036
    max_position_usdt: 20.00
    atr_period: 14
    atr_multiplier: 0.6
  
  trailing_stop:
    enabled: true
    by_profile:
      CONSERVATIVE:
        activate_after_profit_pct: 0.012
        initial_trail_pct: 0.006
        ratchet_levels:
          - at_profit_pct: 0.025
            trail_pct: 0.01
          - at_profit_pct: 0.05
            trail_pct: 0.018
        move_to_break_even_at: 0.018
      MEDIUM:
        activate_after_profit_pct: 0.015
        initial_trail_pct: 0.008
        ratchet_levels:
          - at_profit_pct: 0.03
            trail_pct: 0.012
          - at_profit_pct: 0.06
            trail_pct: 0.02
        move_to_break_even_at: 0.02
      AGGRESSIVE:
        activate_after_profit_pct: 0.02
        initial_trail_pct: 0.01
        ratchet_levels:
          - at_profit_pct: 0.04
            trail_pct: 0.018
          - at_profit_pct: 0.08
            trail_pct: 0.03
        move_to_break_even_at: 0.025
  
  liquidity:
    min_24h_volume_usdt: 60000000
    max_spread_pct: 0.001
    avoid_during_low_liquidity: true
  
  smart_copy:
    recommended_for_beginners: true
    warning: null

profiles:
  CONSERVATIVE:
    risk_per_trade_pct: 0.75
    max_leverage: 2
    max_daily_loss_pct: 0.02
    max_open_positions: 1
    max_total_exposure_pct: 0.30
    trailing_stop_aggressiveness: "high"
    position_sizing_multiplier: 0.6
  
  MEDIUM:
    risk_per_trade_pct: 1.25
    max_leverage: 2
    max_daily_loss_pct: 0.03
    max_open_positions: 2
    max_total_exposure_pct: 0.50
    trailing_stop_aggressiveness: "medium"
    position_sizing_multiplier: 1.0
  
  AGGRESSIVE:
    risk_per_trade_pct: 2.00
    max_leverage: 3
    max_daily_loss_pct: 0.05
    max_open_positions: 3
    max_total_exposure_pct: 0.70
    trailing_stop_aggressiveness: "low"
    position_sizing_multiplier: 1.4
```

---

## 4️⃣ **Risk Management e Perfis de Trading**

### **4.1 Três Perfis de Risco**

| Parâmetro | 🟢 Conservative | 🟡 Medium | 🔴 Aggressive |
|-----------|----------------|-----------|---------------|
| **Risk por Trade** | 0.75% | 1.25% | 2.00% |
| **Stop Loss** | 1.0% | 1.5% | 2.5% |
| **Take Profit** | 2.0% | 3.0% | 5.0% |
| **Risk/Reward** | 2:1 | 2:1 | 2:1 |
| **Max Leverage** | 2x | 2x | 3x |
| **Max Daily Loss** | 2% | 3% | 5% |
| **Max Open Positions** | 1 | 2 | 3 |
| **Max Total Exposure** | 30% | 50% | 70% |
| **ATR Multiplier** | 0.3 | 0.5 | 0.7 |
| **Consecutive Loss Limit** | 2 | 3 | 5 |
| **Time Exit (max)** | 60min | 90min | 120min |

### **4.2 Circuit Breakers Nativos (Tupã v0.8.0)**

```yaml
circuit_breakers:
  # Configurados via @constraints no pipeline Tupã
  native_constraints:
    circuit_breaker: { max_consecutive_failures: 3, timeout_seconds: 300 }
    risk_limit: { max_drawdown_pct: 0.10, daily_loss_pct: 0.05 }
  
  # Comportamento por tipo
  by_type:
    daily_loss_limit:
      Conservative: 2%
      Medium: 3%
      Aggressive: 5%
      action: "pause_new_entries_until_utc_midnight"
    
    consecutive_losses:
      Conservative: 2
      Medium: 3
      Aggressive: 5
      action: "pause_trading_30min"
    
    volatility_spike:
      atr_increase_threshold: 50%
      action: "reduce_position_sizing_50%"
    
    flash_crash:
      price_drop_5min_threshold: 5%
      action: "close_all_longs_pause_trading"
    
    liquidation_cascade:
      market_liquidations_1h_usd: 100000000
      action: "reduce_exposure_50%"
```

### **4.3 Position Sizing (ATR-Adjusted + Smart Copy)**

```yaml
position_sizing:
  method: "atr_adjusted_with_smart_copy_constraints"
  
  base_risk_usd:
    Conservative: 0.75
    Medium: 1.25
    Aggressive: 2.00
  
  atr:
    period: 14
    timeframe: "1h"
  
  smart_copy_constraints:
    min_position_usdt: 5
    max_position_usdt: 30
    max_position_change_between_trades_pct: 0.30
  
  formula: |
    raw_size = (base_risk_usd / (atr_value * leverage)) * atr_multiplier
    final_size = clamp(raw_size, min_position_usdt, max_position_usdt)
    smoothed = apply_smoothing(final_size, last_position_size, max_change_pct)
    return smoothed
```

---

## 5️⃣ **Database Schema**

### **5.1 Tabelas Principais (database/schema.sql)**

```sql
-- database/schema.sql
-- ViperTrade Database Schema v0.8.0
-- Otimizado para Tupã audit logging e Smart Copy tracking

-- ═══════════════════════════════════════════════════════════
-- EXTENSIONS
-- ═══════════════════════════════════════════════════════════

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pg_stat_statements";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- ═══════════════════════════════════════════════════════════
-- TABLE 1: trades (Histórico imutável de operações)
-- ═══════════════════════════════════════════════════════════

CREATE TABLE trades (
    trade_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    bybit_order_id TEXT UNIQUE,
    order_link_id TEXT UNIQUE,
    
    symbol TEXT NOT NULL CHECK (symbol IN ('DOGEUSDT', 'XRPUSDT', 'TRXUSDT', 'XLMUSDT')),
    side TEXT NOT NULL CHECK (side IN ('Long', 'Short')),
    
    quantity NUMERIC NOT NULL CHECK (quantity > 0),
    entry_price NUMERIC NOT NULL CHECK (entry_price > 0),
    exit_price NUMERIC,
    leverage NUMERIC NOT NULL DEFAULT 2 CHECK (leverage >= 1 AND leverage <= 3),
    
    pnl NUMERIC,
    pnl_pct NUMERIC,
    fees NUMERIC DEFAULT 0,
    funding_paid NUMERIC DEFAULT 0,
    slippage_pct NUMERIC DEFAULT 0,
    
    opened_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    closed_at TIMESTAMPTZ,
    duration_seconds INTEGER GENERATED ALWAYS AS (
        EXTRACT(EPOCH FROM (closed_at - opened_at))::INTEGER
    ) STORED,
    
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'closed', 'liquidated', 'cancelled', 'rejected')),
    close_reason TEXT CHECK (close_reason IN ('take_profit', 'stop_loss', 'trailing_stop', 'time_exit', 'manual', 'liquidation', 'circuit_breaker', 'error')),
    
    -- Tupã Engine Audit (CRÍTICO para compliance)
    pipeline_version TEXT NOT NULL DEFAULT '0.8.0',
    decision_hash TEXT NOT NULL,
    execution_hash TEXT,
    constraints_satisfied BOOLEAN DEFAULT true,
    
    -- Smart Copy metadata
    smart_copy_compatible BOOLEAN DEFAULT true,
    copy_ratio NUMERIC DEFAULT 1.0,
    
    -- Trailing stop tracking
    trailing_stop_activated BOOLEAN DEFAULT false,
    trailing_stop_peak_price NUMERIC,
    trailing_stop_final_distance_pct NUMERIC,
    
    trading_profile TEXT NOT NULL DEFAULT 'MEDIUM' CHECK (trading_profile IN ('CONSERVATIVE', 'MEDIUM', 'AGGRESSIVE')),
    
    paper_trade BOOLEAN DEFAULT FALSE,
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_trades_status ON trades(status);
CREATE INDEX idx_trades_symbol ON trades(symbol);
CREATE INDEX idx_trades_opened_at ON trades(opened_at);
CREATE INDEX idx_trades_pnl ON trades(pnl);
CREATE INDEX idx_trades_decision_hash ON trades(decision_hash);
CREATE INDEX idx_trades_paper_trade ON trades(paper_trade);
CREATE INDEX idx_trades_profile ON trades(trading_profile);

-- ═══════════════════════════════════════════════════════════
-- TABLE 2: position_snapshots (Para reconciliação Bybit vs Local)
-- ═══════════════════════════════════════════════════════════

CREATE TABLE position_snapshots (
    snapshot_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    symbol TEXT NOT NULL,
    bybit_data JSONB NOT NULL,
    local_calculation JSONB NOT NULL,
    divergence NUMERIC,
    divergence_pct NUMERIC,
    reconciled BOOLEAN DEFAULT FALSE,
    reconciliation_notes TEXT,
    snapshot_at TIMESTAMPTZ DEFAULT NOW(),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_snapshots_symbol ON position_snapshots(symbol);
CREATE INDEX idx_snapshots_reconciled ON position_snapshots(reconciled);
CREATE INDEX idx_snapshots_created_at ON position_snapshots(created_at);
CREATE INDEX idx_snapshots_divergence ON position_snapshots(divergence_pct) WHERE divergence_pct > 0.01;

-- ═══════════════════════════════════════════════════════════
-- TABLE 3: system_events (Audit trail de todos os eventos)
-- ═══════════════════════════════════════════════════════════

CREATE TABLE system_events (
    event_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_type TEXT NOT NULL,
    severity TEXT NOT NULL DEFAULT 'info' CHECK (severity IN ('debug', 'info', 'warning', 'error', 'critical')),
    category TEXT CHECK (category IN ('trade', 'risk', 'system', 'notification', 'reconciliation', 'tupa', 'circuit_breaker')),
    data JSONB NOT NULL,
    symbol TEXT,
    trade_id UUID REFERENCES trades(trade_id) ON DELETE SET NULL,
    pipeline_version TEXT,
    decision_hash TEXT,
    timestamp TIMESTAMPTZ DEFAULT NOW(),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_events_type ON system_events(event_type);
CREATE INDEX idx_events_severity ON system_events(severity);
CREATE INDEX idx_events_timestamp ON system_events(timestamp);
CREATE INDEX idx_events_symbol ON system_events(symbol);
CREATE INDEX idx_events_category ON system_events(category);
CREATE INDEX idx_events_critical ON system_events(severity, timestamp) WHERE severity IN ('error', 'critical');

-- ═══════════════════════════════════════════════════════════
-- TABLE 4: daily_metrics (Agregados diários para dashboard)
-- ═══════════════════════════════════════════════════════════

CREATE TABLE daily_metrics (
    date DATE PRIMARY KEY,
    total_trades INTEGER DEFAULT 0,
    winning_trades INTEGER DEFAULT 0,
    losing_trades INTEGER DEFAULT 0,
    win_rate NUMERIC DEFAULT 0,
    total_pnl NUMERIC DEFAULT 0,
    total_pnl_pct NUMERIC DEFAULT 0,
    total_fees NUMERIC DEFAULT 0,
    total_funding NUMERIC DEFAULT 0,
    avg_slippage_pct NUMERIC DEFAULT 0,
    max_drawdown NUMERIC DEFAULT 0,
    max_position_size NUMERIC DEFAULT 0,
    circuit_breaker_triggers INTEGER DEFAULT 0,
    consecutive_losses INTEGER DEFAULT 0,
    pipeline_executions INTEGER DEFAULT 0,
    avg_execution_latency_ms NUMERIC DEFAULT 0,
    constraints_violations INTEGER DEFAULT 0,
    copy_success_rate NUMERIC DEFAULT 1.0,
    failed_copies INTEGER DEFAULT 0,
    by_pair JSONB DEFAULT '{}',
    by_profile JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- ═══════════════════════════════════════════════════════════
-- TABLE 5: tupa_audit_logs (Logs estruturados do Tupã Engine)
-- ═══════════════════════════════════════════════════════════

CREATE TABLE tupa_audit_logs (
    log_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    execution_id TEXT NOT NULL,
    pipeline_name TEXT NOT NULL,
    pipeline_version TEXT NOT NULL,
    input_hash TEXT NOT NULL,
    output_hash TEXT NOT NULL,
    decision_hash TEXT NOT NULL,
    input_data JSONB,
    output_data JSONB,
    constraints_results JSONB,
    execution_time_ms INTEGER,
    memory_used_kb INTEGER,
    environment JSONB,
    executed_at TIMESTAMPTZ DEFAULT NOW(),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_audit_execution_id ON tupa_audit_logs(execution_id);
CREATE INDEX idx_audit_pipeline ON tupa_audit_logs(pipeline_name, pipeline_version);
CREATE INDEX idx_audit_timestamp ON tupa_audit_logs(executed_at);
CREATE INDEX idx_audit_decision ON tupa_audit_logs(decision_hash);

-- ═══════════════════════════════════════════════════════════
-- TABLE 6: profile_history (Tracking de mudanças de perfil)
-- ═══════════════════════════════════════════════════════════

CREATE TABLE profile_history (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    profile_code TEXT NOT NULL CHECK (profile_code IN ('CONSERVATIVE', 'MEDIUM', 'AGGRESSIVE')),
    changed_at TIMESTAMPTZ DEFAULT NOW(),
    reason TEXT,
    previous_profile TEXT,
    user_id TEXT,
    metadata JSONB
);

CREATE INDEX idx_profile_history_changed_at ON profile_history(changed_at);

-- ═══════════════════════════════════════════════════════════
-- TABLE 7: circuit_breaker_events (Histórico de circuit breakers)
-- ═══════════════════════════════════════════════════════════

CREATE TABLE circuit_breaker_events (
    event_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    breaker_type TEXT NOT NULL CHECK (breaker_type IN ('daily_loss', 'consecutive_losses', 'volatility_spike', 'flash_crash', 'liquidation_cascade')),
    trigger_value NUMERIC NOT NULL,
    threshold_value NUMERIC NOT NULL,
    action_taken TEXT NOT NULL,
    positions_affected INTEGER DEFAULT 0,
    activated_at TIMESTAMPTZ DEFAULT NOW(),
    deactivated_at TIMESTAMPTZ,
    duration_seconds INTEGER GENERATED ALWAYS AS (
        EXTRACT(EPOCH FROM (deactivated_at - activated_at))::INTEGER
    ) STORED,
    metadata JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_circuit_breaker_type ON circuit_breaker_events(breaker_type);
CREATE INDEX idx_circuit_breaker_activated ON circuit_breaker_events(activated_at);

-- ═══════════════════════════════════════════════════════════
-- TABLE 8: schema_migrations (Versionamento do schema)
-- ═══════════════════════════════════════════════════════════

CREATE TABLE schema_migrations (
    version TEXT PRIMARY KEY,
    applied_at TIMESTAMPTZ DEFAULT NOW(),
    checksum TEXT NOT NULL,
    description TEXT
);

-- ═══════════════════════════════════════════════════════════
-- FUNCTIONS & TRIGGERS
-- ═══════════════════════════════════════════════════════════

CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER update_trades_updated_at
    BEFORE UPDATE ON trades
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_daily_metrics_updated_at
    BEFORE UPDATE ON daily_metrics
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE OR REPLACE FUNCTION calculate_win_rate(start_date DATE, end_date DATE)
RETURNS NUMERIC AS $$
DECLARE
    total INTEGER;
    wins INTEGER;
BEGIN
    SELECT COUNT(*), COUNT(*) FILTER (WHERE pnl > 0)
    INTO total, wins
    FROM trades
    WHERE status = 'closed'
      AND DATE(opened_at) BETWEEN start_date AND end_date;
    
    IF total = 0 THEN RETURN 0; END IF;
    RETURN (wins::NUMERIC / total::NUMERIC) * 100;
END;
$$ LANGUAGE plpgsql;

-- ═══════════════════════════════════════════════════════════
-- VIEWS PARA CONSULTAS COMUNS
-- ═══════════════════════════════════════════════════════════

CREATE OR REPLACE VIEW public_performance AS
SELECT 
    CURRENT_DATE as date,
    COUNT(*) FILTER (WHERE status = 'closed' AND DATE(opened_at) >= CURRENT_DATE - INTERVAL '30 days') as trades_30d,
    COUNT(*) FILTER (WHERE status = 'closed' AND pnl > 0 AND DATE(opened_at) >= CURRENT_DATE - INTERVAL '30 days') as winning_trades_30d,
    ROUND(COUNT(*) FILTER (WHERE status = 'closed' AND pnl > 0 AND DATE(opened_at) >= CURRENT_DATE - INTERVAL '30 days')::NUMERIC / 
          NULLIF(COUNT(*) FILTER (WHERE status = 'closed' AND DATE(opened_at) >= CURRENT_DATE - INTERVAL '30 days'), 0) * 100, 2) as win_rate_30d,
    ROUND(SUM(pnl) FILTER (WHERE status = 'closed' AND DATE(opened_at) >= CURRENT_DATE - INTERVAL '30 days'), 2) as pnl_30d,
    ROUND(SUM(pnl) FILTER (WHERE status = 'closed' AND DATE(opened_at) >= CURRENT_DATE - INTERVAL '30 days') / 
          NULLIF(SUM(ABS(pnl)) FILTER (WHERE status = 'closed' AND DATE(opened_at) >= CURRENT_DATE - INTERVAL '30 days'), 0), 2) as profit_factor_30d,
    ROUND(MIN(pnl_pct) FILTER (WHERE status = 'closed' AND DATE(opened_at) >= CURRENT_DATE - INTERVAL '30 days'), 2) as max_drawdown_30d,
    ROUND(AVG(duration_seconds) FILTER (WHERE status = 'closed' AND DATE(opened_at) >= CURRENT_DATE - INTERVAL '30 days') / 60, 1) as avg_trade_duration_minutes
FROM trades
WHERE paper_trade = false;

CREATE OR REPLACE VIEW pending_reconciliations AS
SELECT 
    ps.symbol,
    ps.snapshot_at,
    ps.divergence_pct,
    ps.bybit_data->>'unrealized_pnl' as bybit_pnl,
    ps.local_calculation->>'unrealized_pnl' as local_pnl,
    ABS((ps.bybit_data->>'unrealized_pnl')::NUMERIC - (ps.local_calculation->>'unrealized_pnl')::NUMERIC) as pnl_diff
FROM position_snapshots ps
WHERE ps.reconciled = false
  AND ps.divergence_pct > 0.01
ORDER BY ps.divergence_pct DESC, ps.snapshot_at DESC;
```

---

## 6️⃣ **Bybit API Integration**

### **6.1 Endpoints Principais**

```yaml
bybit_api:
  base_url_testnet: "https://api-testnet.bybit.com"
  base_url_mainnet: "https://api.bybit.com"
  
  order_endpoints:
    create: "POST /v5/order/create"
    cancel: "POST /v5/order/cancel"
    amend: "POST /v5/order/amend"
    get: "GET /v5/order/realtime"
    history: "GET /v5/order/history"
  
  market_endpoints:
    kline: "GET /v5/market/kline"
    tickers: "GET /v5/market/tickers"
    funding: "GET /v5/market/funding/history"
    orderbook: "GET /v5/market/orderbook"
  
  account_endpoints:
    wallet_balance: "GET /v5/account/wallet-balance"
    positions: "GET /v5/position/list"
    execution_list: "GET /v5/execution/list"
  
  rate_limits:
    general: "600 requests / 5 seconds per IP"
    order_create: "20 orders / second per UID"
    order_amend: "20 orders / second per UID"
  
  websocket:
    public: "wss://stream.bybit.com/v5/public/linear"
    private: "wss://stream.bybit.com/v5/private"
    channels_public:
      - "tickers.{symbol}"
      - "kline.1.{symbol}"
      - "orderbook.50.{symbol}"
      - "funding.{symbol}"
    channels_private:
      - "position"
      - "execution"
      - "order"
      - "wallet"
```

### **6.2 Rate Limiting Implementation**

```yaml
rate_limiting:
  library: "governor"
  strategy: "token_bucket"
  
  service_limits:
    market-data:
      websocket: "no_limit"
      rest_fallback: "100 requests / minute"
    executor:
      order_create: "10 orders / second"
      order_amend: "10 orders / second"
      retry_backoff: "exponential"
    monitor:
      reconciliation: "60 requests / minute"
      health_check: "10 requests / minute"
  
  circuit_breaker:
    trigger: "rate_limit_hit_3_times_in_1min"
    action: "pause_new_orders_5min"
    alert: "warning"
```

---

## 7️⃣ **Error Handling Matrix**

```yaml
error_handling:
  
  bybit_api_errors:
    rate_limit_exceeded:
      http_codes: [429, 403]
      severity: warning
      action: ["backoff_exponential", "retry_5_attempts"]
      alert: "after_3_failures"
    
    authentication_failed:
      http_codes: [401, 403]
      severity: critical
      action: ["stop_trading_immediately", "do_not_retry"]
      alert: "immediate"
    
    order_rejected:
      http_codes: [400]
      severity: warning
      action: ["log_reason", "do_not_retry_same_order"]
      alert: "immediate"
    
    insufficient_balance:
      http_codes: [400]
      severity: critical
      action: ["pause_new_entries", "log_critical"]
      alert: "immediate"
    
    server_error:
      http_codes: [500, 502, 503, 504]
      severity: high
      action: ["backoff_exponential", "retry_10_attempts"]
      alert: "after_5_failures"
  
  websocket_errors:
    disconnected:
      severity: high
      action: ["reconnect_immediately", "exponential_backoff"]
      alert: "after_3_reconnect_failures"
      fallback: "rest_polling_5s_interval"
    
    heartbeat_timeout:
      severity: high
      action: ["force_reconnect", "resubscribe_channels"]
      alert: "after_2_timeouts"
  
  database_errors:
    connection_lost:
      severity: critical
      action: ["buffer_in_memory_max_100", "reconnect_every_5s"]
      alert: "immediate"
    
    write_failed:
      severity: high
      action: ["retry_3_attempts", "buffer_if_fail"]
      alert: "after_5_failures"
  
  risk_errors:
    daily_loss_limit_reached:
      severity: critical
      action: ["pause_all_new_entries", "reset_at_utc_midnight"]
      alert: "immediate"
    
    circuit_breaker_triggered:
      severity: critical
      action: ["close_all_positions", "pause_all_trading", "require_manual_restart"]
      alert: "immediate"
  
  backoff_strategy:
    exponential:
      base_delay_ms: 1000
      max_delay_ms: 30000
      multiplier: 2.0
      jitter: true
      max_attempts: 5
```

---

## 8️⃣ **WebSocket Reconnection Strategy**

```yaml
websocket_reconnection:
  
  public:
    endpoint: "wss://stream.bybit.com/v5/public/linear"
    reconnect:
      enabled: true
      strategy: exponential_backoff
      initial_delay_ms: 1000
      max_delay_ms: 30000
      multiplier: 2.0
      max_attempts: 10
      jitter: true
    heartbeat:
      interval_seconds: 30
      timeout_seconds: 60
    resubscription:
      on_reconnect: resubscribe_all_channels
  
  private:
    endpoint: "wss://stream.bybit.com/v5/private"
    reconnect:
      enabled: true
      strategy: exponential_backoff
      initial_delay_ms: 500
      max_delay_ms: 15000
      multiplier: 2.0
      max_attempts: 15
      jitter: true
    heartbeat:
      interval_seconds: 20
      timeout_seconds: 45
    authentication:
      required: true
      expiry_check: true
      refresh_before_expiry_seconds: 300
  
  fallback:
    enabled: true
    trigger: "websocket_unavailable_for_60_seconds"
    rest_polling:
      positions_interval_seconds: 5
      orders_interval_seconds: 5
      balance_interval_seconds: 10
    alert_on_activate: true
  
  state_recovery:
    verify_position_state: true
    verify_order_state: true
    reconcile_via_rest: true
    max_reconcile_attempts: 3
```

---

## 9️⃣ **Disaster Recovery Procedures**

```yaml
disaster_recovery:
  
  classification:
    critical:
      response_time_minutes: 5
      examples: ["capital_loss_in_progress", "api_key_compromise"]
      actions: ["kill_switch", "close_all_positions", "revoke_api_keys"]
    
    high:
      response_time_minutes: 15
      examples: ["system_outage", "data_corruption"]
      actions: ["restart_services", "restore_from_backup"]
    
    medium:
      response_time_minutes: 60
      examples: ["degraded_performance", "config_error"]
      actions: ["investigate", "apply_workaround"]
    
    low:
      response_time_minutes: 1440
      examples: ["minor_bug", "cosmetic_issue"]
      actions: ["log_for_future_fix"]
  
  recovery_objectives:
    rto:
      critical: 5 minutes
      high: 15 minutes
      medium: 60 minutes
      low: 24 hours
    rpo:
      trades: 0
      positions: 5 minutes
      balance: 1 minute
      logs: 60 minutes
  
  procedures:
    kill_switch:
      triggers:
        - "daily_loss > 5%"
        - "consecutive_losses > 5"
        - "api_key_compromise_suspected"
      actions:
        - "cancel_all_pending_orders"
        - "close_all_open_positions_at_market"
        - "pause_all_new_entries"
        - "send_critical_alert_all_channels"
    
    database_restore:
      backup_frequency: "daily_at_00:00_UTC"
      retention_days: 30
      restore_steps:
        - "stop_all_services"
        - "restore_from_backup"
        - "start_services"
        - "reconcile_with_bybit"
    
    api_key_compromise:
      indicators:
        - "unauthorized_orders_detected"
        - "withdrawal_attempts"
      immediate_actions:
        - "kill_switch_immediate"
        - "revoke_api_key_on_bybit"
        - "change_bybit_password"
        - "enable_2fa"
  
  post_mortem:
    required_for: ["critical", "high"]
    template:
      - "incident_summary"
      - "timeline"
      - "impact"
      - "root_cause"
      - "remediation"
      - "action_items"
      - "lessons_learned"
    draft_within_hours: 48
    review_within_hours: 72
```

---

## 🔐 **10. Secrets Management**

```yaml
secrets_management:
  
  storage:
    method: ".env file with chmod 600"
    location: "compose/.env"
    git_protection: "added to .gitignore"
  
  permissions:
    file: 600
    directory: 700
    scripts: +x
  
  rotation:
    api_keys:
      frequency_days: 90
      alert_before_days: 7
      procedure:
        - "Create new API key on Bybit"
        - "Test new key on testnet"
        - "Update compose/.env"
        - "Restart services"
        - "Validate for 1 hour"
        - "Revoke old key"
        - "Document rotation"
    
    bybit_api_permissions:
      - "Order: Enable"
      - "Position: Enable"
      - "Account: Read-only"
      - "Withdraw: DISABLED (CRITICAL)"
    
    ip_whitelist:
      enabled: true
      add_server_ip: true
      update_on_ip_change: true
    
    two_factor_auth:
      required: true
      enabled_on_account: true
  
  security_audit:
    pre_mainnet_checklist:
      - "API keys with minimal permissions"
      - "Withdraw permission DISABLED"
      - "IP whitelist configured"
      - "2FA enabled on Bybit account"
      - ".env with permission 600"
      - ".env NOT in Git"
      - "security-check.sh passes"
      - "Kill switch tested"
      - "Emergency procedures documented"
      - "Database backup configured"
      - "Critical alerts tested"
      - "Paper trading 21 days stable"
      - "Backtest approved on stress scenarios"
    
    code_audit:
      - "No secrets hardcoded"
      - "All inputs validated"
      - "Error messages don't leak info"
      - "Rate limiting implemented"
      - "SQL injection protected"
      - "XSS protected (web)"
      - "CSRF protected (web)"
      - "Dependencies updated (cargo audit)"
```

---

## 1️⃣1️⃣ **Discord Notifications**

```yaml
discord:
  enabled: true
  
  webhooks:
    critical: "DISCORD_WEBHOOK_CRITICAL"
    warning: "DISCORD_WEBHOOK_WARNING"
    info: "DISCORD_WEBHOOK_INFO"
  
  settings:
    timeout_seconds: 10
    retry_attempts: 3
    retry_backoff_seconds: 2
    rate_limit:
      max_per_minute: 30
      max_per_hour: 500
    batching:
      enabled: true
      interval_seconds: 900
      max_batch_size: 10
    fallback:
      enabled: true
      method: "email"
      on_consecutive_failures: 3
  
  formatting:
    use_embeds: true
    show_timestamp: true
    color_scheme:
      critical: 0xFF0000
      warning: 0xFFA500
      info: 0x00FF00
      success: 0x00AA00
    bot:
      username: "ViperTrade Bot"
  
  filters:
    quiet_hours:
      enabled: false
      start_utc: "02:00"
      end_utc: "08:00"
      allow_critical: true
    deduplication:
      enabled: true
      window_seconds: 300

alert_templates:
  
  circuit_breaker:
    priority: critical
    title: "🚨 CIRCUIT BREAKER ACTIVATED"
    color: 0xFF0000
    fields:
      - name: "Motivo"
        value: "{reason}"
      - name: "P&L Atual"
        value: "{pnl}"
      - name: "Ação"
        value: "Trading pausado até 00:00 UTC"
  
  stop_loss:
    priority: warning
    title: "⚠️ STOP LOSS ACIONADO"
    color: 0xFFA500
    fields:
      - name: "Símbolo"
        value: "{symbol}"
      - name: "Lado"
        value: "{side}"
      - name: "Perda"
        value: "{loss}"
  
  trailing_stop_activated:
    priority: info
    title: "📈 TRAILING STOP ATIVADO"
    color: 0x00FF00
    fields:
      - name: "Símbolo"
        value: "{symbol}"
      - name: "Lucro Atual"
        value: "{profit_pct}"
      - name: "Trail Distance"
        value: "{trail_pct}"
  
  trade_executed:
    priority: info
    title: "✅ TRADE EXECUTADO"
    color: 0x00FF00
    fields:
      - name: "Símbolo"
        value: "{symbol}"
      - name: "Tamanho"
        value: "${size}"
      - name: "Entrada"
        value: "${entry_price}"
  
  daily_summary:
    priority: info
    title: "📊 RESUMO DIÁRIO"
    color: 0x3B82F6
    fields:
      - name: "P&L Total"
        value: "{total_pnl}"
      - name: "Trades"
        value: "{trade_count}"
      - name: "Win Rate"
        value: "{win_rate}"

alert_recipients:
  
  discord_alerts:
    recipient: "Bot operator (you)"
    purpose: "Monitor bot health, performance, and take action if needed"
    critical_alerts:
      - "Circuit breaker triggered"
      - "API connection lost"
      - "Daily loss limit reached"
      - "Reconciliation divergence detected"
      - "Trailing stop activated"
  
  bybit_email_alerts:
    recipient: "Followers copying ViperTrade"
    purpose: "Notify followers of copy-specific events (sent by Bybit)"
    note: "We cannot control these; they are sent by Bybit platform"
  
  operational_implications:
    do_not_rely_on_followers_seeing_our_alerts: true
    public_risk_documentation: true
    monitor_public_metrics: true
```

---

## 1️⃣2️⃣ **Tupã Language Integration (crates.io v0.8.0)**

### **12.1 Versioning Strategy**

```yaml
tupa_integration:
  
  version: "0.8.0"
  source: "crates.io"
  license: "Apache-2.0/MIT"
  
  crates:
    - name: "tupa-runtime"
      version: "0.8.0"
      features: ["trading", "audit", "backtest"]
      purpose: "Pipeline execution engine"
    
    - name: "tupa-codegen"
      version: "0.8.0"
      purpose: "ExecutionPlan generation (JSON/LLVM)"
    
    - name: "tupa-audit"
      version: "0.8.0"
      purpose: "Structured audit logging"
    
    - name: "tupa-effects"
      version: "0.8.0"
      purpose: "Controlled side effects (IO/Random)"
    
    - name: "tupa-parser"
      version: "0.8.0"
      purpose: "Source code parsing"
    
    - name: "tupa-typecheck"
      version: "0.8.0"
      purpose: "Static type checking"
  
  compatibility:
    vipertrade_0.8.0:
      tupa_min: "0.8.0"
      tupa_max: "0.8.x"
      rust_min: "1.75"
  
  update_policy:
    check_crates_io: weekly
    allow_patch_upgrades: true
    require_review_for_minor: true
    pin_exact_version_for_prod: true
```

### **12.2 Pipeline Tupã para Smart Copy (config/strategies/viper_smart_copy.tp)**

```tupã
// config/strategies/viper_smart_copy.tp
// ViperTrade v0.8.0 - Smart Copy Pipeline com Trailing Stop Dinâmico
// Compatível com Tupã v0.8.0 (crates.io, hybrid backend)

@use tupa::constraints::{CircuitBreaker, RiskLimit, DrawdownLimit};
@use tupa::audit::{log_decision, AuditLevel};
@use tupa::effects::{IO, State};

type MarketSignal {
    symbol: string,
    current_price: f64,
    atr_14: f64,
    volume_24h: i64,
    funding_rate: f64,
    trend_score: f64,
    spread_pct: f64,
}

type SmartCopyConfig {
    profile: string,
    risk_per_trade_pct: f64,
    stop_loss_pct: f64,
    take_profit_pct: f64,
    max_leverage: f64,
    min_position_usdt: f64,
    max_position_usdt: f64,
    max_position_change_pct: f64,
}

type TrailingConfig {
    activate_after_profit_pct: f64,
    initial_trail_pct: f64,
    ratchet_levels: [RatchetLevel],
    move_to_break_even_at: f64,
}

type RatchetLevel {
    at_profit_pct: f64,
    trail_pct: f64,
}

type TradeDecision {
    action: string,
    symbol: string,
    quantity: f64,
    leverage: f64,
    entry_price: f64,
    stop_loss: f64,
    take_profit: f64,
    reason: string,
    smart_copy_compatible: bool,
    trailing_config: TrailingConfig,
}

extern fn viper::calculate_smart_copy_size(signal: MarketSignal, config: SmartCopyConfig, last_position_size: f64) -> f64;
extern fn viper::check_smart_copy_constraints(size: f64, config: SmartCopyConfig) -> bool;
extern fn viper::validate_entry_conditions(signal: MarketSignal, config: SmartCopyConfig) -> bool;
extern fn viper::calculate_dynamic_trail(current_profit_pct: f64, config: TrailingConfig) -> f64;
extern fn viper::get_trailing_config(profile: string, symbol: string) -> TrailingConfig;
extern fn viper::check_daily_loss(current_loss: f64, max_daily_loss_pct: f64) -> bool;
extern fn viper::check_consecutive_losses(count: i64, max_count: i64) -> bool;
extern fn viper::validate_funding_rate(funding: f64, side: string, max_rate: f64) -> bool;
extern fn viper::check_liquidation_distance(entry: f64, leverage: f64, side: string, min_distance: f64) -> bool;

@constraints(
    circuit_breaker = { max_consecutive_failures: 3, timeout_seconds: 300 },
    risk_limit = { max_drawdown_pct: 0.10, daily_loss_pct: 0.05 },
    audit_level = "detailed"
)

pipeline ViperSmartCopy @deterministic(seed=42) {
    input: MarketSignal,
    
    steps: [
        step("check_daily_loss") {
            viper::check_daily_loss(current_daily_loss, config.max_daily_loss_pct)
        },
        
        step("check_consecutive_losses") {
            viper::check_consecutive_losses(consecutive_losses, 3)
        },
        
        step("validate_entry") {
            viper::validate_entry_conditions(input, config)
        },
        
        step("check_funding") {
            let side = if input.trend_score > 0 { "Long" } else { "Short" };
            viper::validate_funding_rate(input.funding_rate, side, 0.015)
        },
        
        step("calc_smart_size") {
            viper::calculate_smart_copy_size(input, config, last_position_size)
        },
        
        step("validate_size") {
            viper::check_smart_copy_constraints(calc_smart_size, config)
        },
        
        step("get_trailing_config") {
            viper::get_trailing_config(config.profile, input.symbol)
        },
        
        step("decision") {
            if check_daily_loss && 
               check_consecutive_losses && 
               validate_entry && 
               check_funding && 
               validate_size && 
               calc_smart_size >= config.min_position_usdt {
                
                let side = if input.trend_score > 0 { "Long" } else { "Short" };
                let entry = input.current_price;
                let sl = if side == "Long" {
                    entry * (1 - config.stop_loss_pct)
                } else {
                    entry * (1 + config.stop_loss_pct)
                };
                let tp = if side == "Long" {
                    entry * (1 + config.take_profit_pct)
                } else {
                    entry * (1 - config.take_profit_pct)
                };
                
                {
                    action: if side == "Long" { "ENTER_LONG" } else { "ENTER_SHORT" },
                    symbol: input.symbol,
                    quantity: calc_smart_size,
                    leverage: config.max_leverage,
                    entry_price: entry,
                    stop_loss: sl,
                    take_profit: tp,
                    reason: "smart_copy_optimized",
                    smart_copy_compatible: true,
                    trailing_config: get_trailing_config
                }
            } else {
                {
                    action: "HOLD",
                    symbol: input.symbol,
                    quantity: 0,
                    leverage: 0,
                    entry_price: 0,
                    stop_loss: 0,
                    take_profit: 0,
                    reason: "risk_constraints_not_met",
                    smart_copy_compatible: false,
                    trailing_config: get_trailing_config
                }
            }
        },
        
        step("audit") {
            log_decision(decision, level: AuditLevel::Detailed)
        }
    ],
    
    @validate {
        assert(decision.action == "HOLD" || decision.stop_loss > 0);
        
        let risk = (decision.entry_price - decision.stop_loss).abs();
        let reward = (decision.take_profit - decision.entry_price).abs();
        assert(decision.action == "HOLD" || (reward / risk) >= 2.0);
        
        let position_value = decision.quantity * decision.entry_price;
        assert(decision.action == "HOLD" || 
               (position_value >= config.min_position_usdt && 
                position_value <= config.max_position_usdt));
        
        assert(decision.trailing_config.activate_after_profit_pct > 0);
    },
    
    output: {
        decision: decision,
        execution_hash: hash(decision),
        timestamp: now(),
        pipeline_version: "0.8.0",
        smart_copy_mode: true,
        constraints_satisfied: true
    }
}
```

### **12.3 Integração Rust com Tupã crates.io**

```rust
// services/strategy/src/tupa_engine.rs
use tupa_runtime::{Engine, ExecutionPlan, Value, constraints::CircuitBreaker};
use tupa_audit::{AuditLogger, AuditLevel, AuditEntry};
use tupa_codegen::{compile_pipeline, generate_plan};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TupaConfig {
    pub pipeline_path: String,
    pub audit_path: String,
    pub seed: Option<u64>,
    pub profile: String,
}

pub struct ViperTupaEngine {
    engine: Engine,
    plan: ExecutionPlan,
    circuit_breaker: CircuitBreaker,
    audit_logger: AuditLogger,
}

impl ViperTupaEngine {
    pub fn new(config: TupaConfig) -> Result<Self, TupaError> {
        // Carregar ExecutionPlan (auto-detect JSON/LLVM backend)
        let plan = generate_plan(&config.pipeline_path)?;
        
        // Inicializar engine com seed para determinismo
        let engine = Engine::new(config.seed)?;
        
        // Circuit breaker nativo configurado via @constraints
        let circuit_breaker = CircuitBreaker::from_pipeline(&plan)?;
        
        // Audit logger nativo
        let audit_logger = AuditLogger::new(&config.audit_path, AuditLevel::Detailed)?;
        
        // Registrar funções externas (Rust)
        engine.register_function("viper::calculate_smart_copy_size", calculate_smart_copy_size)?;
        engine.register_function("viper::check_smart_copy_constraints", check_smart_copy_constraints)?;
        engine.register_function("viper::validate_entry_conditions", validate_entry_conditions)?;
        engine.register_function("viper::calculate_dynamic_trail", calculate_dynamic_trail)?;
        engine.register_function("viper::get_trailing_config", get_trailing_config)?;
        engine.register_function("viper::check_daily_loss", check_daily_loss)?;
        engine.register_function("viper::check_consecutive_losses", check_consecutive_losses)?;
        engine.register_function("viper::validate_funding_rate", validate_funding_rate)?;
        engine.register_function("viper::check_liquidation_distance", check_liquidation_distance)?;
        
        Ok(Self { 
            engine, 
            plan, 
            circuit_breaker,
            audit_logger 
        })
    }
    
    pub fn execute(&self, signal: MarketSignal, config: SmartCopyConfig) -> Result<TradeDecision, TupaError> {
        // Verificar circuit breaker antes de executar
        self.circuit_breaker.check()?;
        
        // Converter input para Value Tupã
        let input = self.signal_to_value(signal, config)?;
        
        // Executar pipeline
        let output = self.engine.execute(&self.plan, input)?;
        
        // Converter output para TradeDecision
        let decision = self.value_to_decision(output)?;
        
        // Audit logging nativo
        self.audit_logger.log(AuditEntry {
            pipeline: "ViperSmartCopy",
            version: "0.8.0",
            action: decision.action.clone(),
            symbol: decision.symbol.clone(),
            quantity: decision.quantity,
            entry_price: decision.entry_price,
            stop_loss: decision.stop_loss,
            take_profit: decision.take_profit,
            smart_copy_compatible: decision.smart_copy_compatible,
            execution_hash: output.get("execution_hash").unwrap().as_str().unwrap(),
            constraints_satisfied: output.get("constraints_satisfied").unwrap().as_bool().unwrap(),
        })?;
        
        // Registrar sucesso/falha no circuit breaker
        if decision.action != "HOLD" {
            self.circuit_breaker.record_success();
        }
        
        Ok(decision)
    }
    
    pub fn backtest(&self, historical_data: Vec<MarketSignal>, config: SmartCopyConfig) -> Result<BacktestResult, TupaError> {
        use tupa_runtime::backtest::Backtester;
        
        let mut backtester = Backtester::new(&self.engine, &self.plan)?;
        
        for signal in historical_data {
            let input = self.signal_to_value(signal.clone(), config.clone())?;
            backtester.step(input)?;
        }
        
        Ok(backtester.finalize())
    }
    
    fn signal_to_value(&self, signal: MarketSignal, config: SmartCopyConfig) -> Result<Value, TupaError> {
        Ok(Value::Object(serde_json::json!({
            "symbol": signal.symbol,
            "current_price": signal.current_price,
            "atr_14": signal.atr_14,
            "volume_24h": signal.volume_24h,
            "funding_rate": signal.funding_rate,
            "trend_score": signal.trend_score,
            "spread_pct": signal.spread_pct,
            "config": config,
        })))
    }
    
    fn value_to_decision(&self, value: Value) -> Result<TradeDecision, TupaError> {
        let decision: TradeDecision = serde_json::from_value(value.into())?;
        Ok(decision)
    }
}

#[derive(Debug)]
pub enum TupaError {
    PipelineNotFound(String),
    PlanGenerationFailed(String),
    ExecutionFailed(String),
    ConstraintViolation(String),
    SerializationError(String),
    DatabaseError(String),
}
```

---

## 1️⃣3️⃣ **Lead Trader Operations**

### **13.1 Bybit Copy Trading Classic Integration**

```yaml
lead_trader_operations:
  
  registration:
    platform: "Bybit Copy Trading Classic"
    role: "Lead Trader"
    requirements:
      - "KYC Level 1+"
      - "Minimum trading experience"
      - "Initial capital deposited"
  
  public_metrics:
    displayed_on_profile:
      - "win_rate_30d"
      - "roi_30d"
      - "roi_all_time"
      - "max_drawdown"
      - "total_followers"
      - "total_aum_copied"
      - "total_trades"
      - "avg_trade_duration"
  
  master_trader_rank:
    ranks:
      cadet:
        min_followers: 0
        min_aum: 0
        max_fixed_margin_per_trade: 1000
      bronze:
        min_followers: 10
        min_aum: 1000
        min_trading_days: 7
        max_fixed_margin_per_trade: 2000
      silver:
        min_followers: 50
        min_aum: 10000
        min_trading_days: 30
        min_win_rate: 0.50
        max_fixed_margin_per_trade: 10000
      gold:
        min_followers: 200
        min_aum: 100000
        min_trading_days: 90
        min_win_rate: 0.55
        max_drawdown: 0.15
        max_fixed_margin_per_trade: "customized"
    
    progression_strategy:
      phase_1_cadet:
        duration_days: 7
        focus: "Consistency over returns"
        max_position_usdt: 5
      phase_2_bronze:
        duration_days: 30
        focus: "Steady growth with controlled risk"
        max_position_usdt: 10
      phase_3_silver:
        duration_days: 60
        focus: "Optimize public metrics"
        max_position_usdt: 20
      phase_4_gold:
        duration_days: 90+
        focus: "Scale with proven strategy"
        max_position_usdt: 50
```

### **13.2 Auto-Unfollow Prevention**

```yaml
auto_unfollow_prevention:
  
  bybit_triggers:
    - "20 consecutive failed copies due to insufficient order cost"
    - "100 failed copies within 24 hours"
  
  failed_copy_causes:
    - "Insufficient follower balance"
    - "Order cost + leverage doesn't meet minimum"
    - "Slippage exceeded follower's max setting"
  
  prevention_strategies:
    position_sizing:
      min_order_cost_usdt: 10
      max_order_cost_usdt: 2000
      avoid_micro_positions: true
    
    slippage_management:
      max_acceptable_slippage_pct: 0.01
      avoid_trading_during_high_volatility: true
      use_limit_orders_when_possible: true
    
    leverage_changes:
      max_changes_per_day: 1
      avoid_changes_during_open_positions: true
      document_rationale_publicly: true
    
    monitoring:
      track_failed_copy_rate: true
      alert_if_failure_rate_gt: 0.05
      auto_reduce_position_size_if_failures_high: true
```

### **13.3 Order Limits Awareness**

```yaml
follower_order_limits:
  
  bybit_limit:
    cumulative_order_value_per_pair: 300000
    calculation: "Sum of (order_cost × leverage) for all open positions"
  
  our_strategy:
    target_follower_profile:
      typical_investment: 100-1000
      typical_leverage: 5-10x
      implied_max_order_cost: 100-200
    
    our_max_position_usdt: 200
    rationale: "Larger positions still copied by larger followers; smaller positions accessible to all"
```

---

## 1️⃣4️⃣ **Backtesting Engine**

```yaml
backtesting:
  
  architecture:
    data_source: "PostgreSQL + CSV histórico"
    execution_mode: "simulated"
    slippage_model: "realistic"
    fee_model: "bybit_actual_fees"
  
  data_requirements:
    - "OHLCV 1min (mínimo 1 ano)"
    - "Funding rates histórico"
    - "Liquidation data"
  
  validation_metrics:
    total_return: "> 10%"
    max_drawdown: "< 10%"
    sharpe_ratio: "> 1.5"
    win_rate: "> 50%"
    profit_factor: "> 1.5"
  
  required_scenarios:
    - "feb_2026_correction (-14%)"
    - "crypto_winter_2022 (-76%)"
    - "covid_crash_2020 (-50%)"
    - "normal_market (3 meses)"
  
  implementation:
    language: "Rust"
    location: "services/backtest/"
    cli_command: "cargo run --backtest --config <profile> --dates <start> <end>"
    output_format: "JSON + HTML report"
```

### **14.2 Paper Trading Mode**

```yaml
paper_trading:
  
  mode: "simulation_with_real_data"
  
  mechanics:
    - "Usa dados reais de mercado (WebSocket Bybit)"
    - "Simula execuções com slippage realista (0.3%)"
    - "Registra no database como trades normais"
    - "Flag 'paper_trade: true' em todos os registros"
    - "Não envia ordens reais para Bybit"
  
  configuration:
    enabled: true
    duration_days: 21
    initial_balance_usdt: 100
    slippage_simulation_pct: 0.003
    fee_simulation_pct: 0.0006
  
  approval_criteria:
    - "21 dias consecutivos sem daily loss violation"
    - "Win rate >= 50%"
    - "Max drawdown <= 5%"
    - "All alerts tested and working"
    - "Reconciliation divergence < 1%"
  
  implementation:
    flag_in_config: "TRADING_MODE=paper"
    executor_behavior: "simulate_only"
    database_flag: "paper_trade: true"
    dashboard_indicator: "📄 PAPER MODE"
```

---

## 1️⃣5️⃣ **Smart Copy Mode Optimization**

```yaml
smart_copy_advantages:
  - "Sizing automático baseado em proporção de capital"
  - "Leverage alinhado automaticamente"
  - "Menos falhas de copy = menos risco de auto-unfollow"
  - "Mais amigável para followers = maior adoção"
  - "Métricas públicas mais consistentes"

smart_copy_mechanics:
  formula: |
    Follower_Position_Size = Master_Position_Size × 
                            (Follower_Equity / Master_Equity) × 
                            Copy_Ratio
  
  example:
    master_equity: 100
    follower_equity: 500
    master_position: 10
    copy_ratio: 1.0
    follower_position: 50

smart_copy_settings:
  
  position_sizing:
    min_position_usdt: 5
    max_position_usdt: 30
    target_position_usdt: 10-20
    max_position_change_between_trades_pct: 0.30
  
  leverage:
    by_profile:
      CONSERVATIVE: 2
      MEDIUM: 2
      AGGRESSIVE: 3
    max_changes_per_week: 2
    require_cool_down_hours: 12
  
  entry_timing:
    avoid_hours_utc: ["00:00-02:00", "22:00-23:59"]
    avoid_conditions:
      - "ATR > 5%"
      - "Spread > 0.1%"
      - "Volume 24h < $30M"
      - "Funding rate > 1.5%"
    order_type_preference: "Limit"
    fallback_to_market: true
    max_slippage_for_market: 0.01
  
  exit_strategy:
    mandatory_stops: true
    trailing_stop:
      enabled: true
      activate_after_profit_pct: 0.015
      initial_offset_pct: 0.008
      dynamic_tightening: true
    time_exit:
      max_hold_minutes: 90
      reduce_time_if_in_loss: true
```

---

## 1️⃣6️⃣ **Dynamic Trailing Stop**

```yaml
trailing_stop_philosophy: |
  "Proteger capital inicialmente, proteger lucros agressivamente depois"
  
advantages_over_fixed:
  - "Protege lucros progressivamente"
  - "Deixa winners correrem"
  - "Reduz emocional"
  - "Smart Copy compatible"
  - "Perfil-customizável"
  - "Break-even protection"

trailing_stop_by_profile:
  
  CONSERVATIVE:
    activate_after_profit_pct: 0.01
    initial_trail_pct: 0.005
    ratchet_levels:
      - at_profit_pct: 0.02
        trail_pct: 0.008
      - at_profit_pct: 0.04
        trail_pct: 0.012
      - at_profit_pct: 0.06
        trail_pct: 0.02
    move_to_break_even_at: 0.015
  
  MEDIUM:
    activate_after_profit_pct: 0.015
    initial_trail_pct: 0.008
    ratchet_levels:
      - at_profit_pct: 0.03
        trail_pct: 0.012
      - at_profit_pct: 0.06
        trail_pct: 0.02
      - at_profit_pct: 0.10
        trail_pct: 0.035
    move_to_break_even_at: 0.02
  
  AGGRESSIVE:
    activate_after_profit_pct: 0.02
    initial_trail_pct: 0.01
    ratchet_levels:
      - at_profit_pct: 0.05
        trail_pct: 0.02
      - at_profit_pct: 0.10
        trail_pct: 0.035
      - at_profit_pct: 0.15
        trail_pct: 0.05
    move_to_break_even_at: 0.03

ratchet_mechanism:
  
  how_it_works: |
    1. Trade entra com stop loss inicial (ex: -1.5%)
    2. Quando preço sobe X%, trailing stop ATIVA
    3. Trail começa com Y% de distância do pico
    4. Conforme preço continua subindo, trail DIMINUI (mais apertado)
    5. Cada nível de lucro "trava" o trail em novo patamar
    6. Trail NUNCA volta para trás (só aperta ou mantém)
  
  example_long:
    entry_price: 100.00
    initial_stop: 98.50
    
    at_1_5_pct:
      action: "Activate trailing"
      trail_distance: 0.8%
      stop_moves_to: 100.68
    
    at_3_pct:
      action: "Tighten to 1.2%"
      trail_distance: 1.2%
      stop_moves_to: 101.76
    
    at_6_pct:
      action: "Tighten to 2.0%"
      trail_distance: 2.0%
      stop_moves_to: 103.88
    
    at_10_pct:
      action: "Tighten to 3.5%"
      trail_distance: 3.5%
      stop_moves_to: 106.15
    
    final_result:
      exit_price: 106.15
      total_profit: 6.15%
      protected_from_peak: 3.5%
      improvement_vs_fixed_tp: "+105%"

implementation:
  
  monitoring:
    check_interval_ms: 500
    track_peak_price: true
    trail_only_moves_up: true
  
  stop_update:
    method: "amend_order"
    min_move_threshold_pct: 0.002
    max_updates_per_minute: 10
    batch_updates: true
  
  fallback:
    if_amend_fails: "Log error + continue monitoring + retry next level"
    alert_operator: true
    manual_override_available: true
  
  smart_copy_compatibility:
    follower_benefit: "Followers recebem mesmo trailing proporcionalmente"
    follower_settings:
      own_trailing_stop: "20% (opcional)"
      pcsl_recommendation: "20-25%"
    avoid_conflicts: "Não usar trailing muito apertado + PCSL muito apertado"
```

---

## 1️⃣7️⃣ **Blocos de Desenvolvimento**

```markdown
## Bloco 0: Preparação (Dia 0)
- [ ] Criar branch develop/v0.8.0
- [ ] Atualizar VERSION para 0.8.0
- [ ] Atualizar VIPERTRADE_SPEC.md com Tupã crates.io integration
- [ ] Criar COMPATIBILITY.md com guia de migração

## Bloco 1: Setup do Projeto (Dia 1-2)
- [ ] Criar estrutura de diretórios
- [ ] Inicializar Git repository
- [ ] Criar .gitignore
- [ ] Criar README.md
- [ ] Criar compose/.env.example
- [ ] Criar scripts/init-secrets.sh
- [ ] Criar scripts/security-check.sh
- [ ] Testar init-secrets.sh

## Bloco 2: Configuração do Ambiente (Dia 2-3)
- [ ] Atualizar compose/.env.example com Tupã v0.8.0 vars
- [ ] Atualizar scripts/init-secrets.sh para Tupã
- [ ] Criar scripts/build-tupa.sh simplificado
- [ ] Testar todos os scripts

## Bloco 3: Docker Compose + Database (Dia 3-5)
- [ ] Criar compose/docker-compose.yml com 8 serviços
- [ ] Configurar volumes persistentes e health checks
- [ ] Criar database/schema.sql com 8 tabelas
- [ ] Criar database/init.sql com dados iniciais
- [ ] Criar scripts/validate-db.sh

## Bloco 4: Configuração dos Pares + Tupã Pipeline (Dia 5-7)
- [ ] Criar config/trading/pairs.yaml com 4 pares
- [ ] Criar config/strategies/viper_smart_copy.tp com sintaxe v0.8.0
- [ ] Criar config/system/profiles.yaml com 3 perfis
- [ ] Criar scripts/validate-pipeline.sh
- [ ] Validar pipeline com tupa check

## Bloco 5: Market Data Service (Dia 7-10)
- [ ] Criar services/market-data/
- [ ] Implementar WebSocket client Bybit
- [ ] Subscrever canais públicos (4 pares)
- [ ] Normalizar dados e publicar no Redis
- [ ] Implementar reconnect strategy
- [ ] Criar Dockerfile

## Bloco 6: Strategy Engine (Dia 10-14)
- [ ] Criar services/strategy/
- [ ] Integrar Tupã v0.8.0 via crates.io
- [ ] Implementar Risk Manager module
- [ ] Implementar position sizing (ATR)
- [ ] Implementar Dynamic Trailing Stop
- [ ] Integrar circuit breaker nativo
- [ ] Criar Dockerfile

## Bloco 7: Order Executor (Dia 14-18)
- [ ] Criar services/executor/
- [ ] Implementar Bybit REST API client
- [ ] Suporte a OCO orders (stop loss + take profit)
- [ ] Implementar retry logic com backoff
- [ ] Rate limiting com governor
- [ ] Reconciliation local vs Bybit
- [ ] Criar Dockerfile

## Bloco 8: Monitor Service (Dia 18-22)
- [ ] Criar services/monitor/
- [ ] Implementar health checks
- [ ] Implementar Discord webhook client
- [ ] Implementar reconciliation engine
- [ ] Implementar Trailing Stop Monitor
- [ ] Integrar tupa_audit para logging
- [ ] Criar Dockerfile

## Bloco 9: Error Handling (Dia 22-24)
- [ ] Implementar error handling matrix
- [ ] Criar error types estruturados
- [ ] Implementar backoff strategies
- [ ] Testar cada tipo de erro

## Bloco 10: Testing & Validation (Dia 24-30)
- [ ] Configurar testnet environment
- [ ] Rodar paper trading 14 dias
- [ ] Backtest com dados históricos
- [ ] Validar risk management
- [ ] Documentar resultados

## Bloco 11: Documentation (Dia 30-32)
- [ ] Escrever ARCHITECTURE.md
- [ ] Escrever OPERATIONS.md
- [ ] Documentar procedimentos de emergência
- [ ] Atualizar README.md

## Bloco 12: Mainnet Micro Deployment (Dia 32+)
- [ ] Criar API keys mainnet
- [ ] Deploy em produção
- [ ] Monitoramento intensivo 72h
- [ ] Ajustar parâmetros se necessário

## Bloco 13: Web Dashboard (Opcional, Dia 32-42)
- [ ] Criar estrutura web/
- [ ] Configurar Next.js 14
- [ ] Implementar páginas principais
- [ ] Integrar WebSocket real-time
- [ ] Criar Dockerfile

## Bloco 14: Smart Copy Optimization (Opcional, Dia 42-45)
- [ ] Configurar smart_copy_optimized.yaml
- [ ] Implementar position smoothing
- [ ] Testar com followers de teste
- [ ] Documentar para followers

## Bloco 15: Dynamic Trailing Stop Validation (Opcional, Dia 45-47)
- [ ] Implementar DynamicTrailingStop struct
- [ ] Configurar ratchet levels por perfil
- [ ] Integrar com Bybit amend_order
- [ ] Backtest trailing dinâmico vs fixo
```

---

## 1️⃣8️⃣ **Checklist de Validação**

```bash
# Pré-Deploy Checklist

# Segurança
[ ] ./scripts/security-check.sh passa
[ ] compose/.env tem permissão 600
[ ] .env NÃO está no Git
[ ] API keys com permissões mínimas
[ ] 2FA ativado na conta Bybit
[ ] IP whitelist configurado

# Database
[ ] Schema aplicado corretamente
[ ] Todas as tabelas existem
[ ] Índices criados
[ ] Backup configurado

# Serviços
[ ] Todos os containers iniciam
[ ] Health checks passam
[ ] Logs estruturados em JSON
[ ] Redis Pub/Sub funciona
[ ] WebSocket reconecta após falha

# Risk Management
[ ] Position sizing ATR-adjusted calcula corretamente
[ ] Stop loss monitora posições
[ ] Dynamic Trailing Stop ativa e ajusta
[ ] Circuit breakers ativam nos thresholds
[ ] Daily loss limit funciona

# Notifications
[ ] Discord webhooks configurados
[ ] Alertas críticos chegam
[ ] Alertas warning chegam
[ ] Batch de info funciona

# Testing
[ ] Paper trading 21 dias estável
[ ] Backtest com dados de stress aprovado
[ ] Error handling testado
[ ] Kill switch testado

# Smart Copy
[ ] Position sizing dentro de $5-$30
[ ] Smoothing de position sizing funciona
[ ] Leverage definido por perfil
[ ] Entry timing evita condições adversas

# Tupã Integration
[ ] Pipeline compila com tupa check
[ ] ExecutionPlan gera com tupa codegen
[ ] Audit logging funciona
[ ] Circuit breaker nativo integrado

# Lead Trader
[ ] Conta Bybit verificada (KYC)
[ ] Copy Trading Leader registrado
[ ] Métricas públicas visíveis
[ ] Guia para followers disponível
```

---

## 1️⃣9️⃣ **Comandos e Scripts Úteis**

```bash
# Inicialização
./scripts/init-secrets.sh
./scripts/security-check.sh
./scripts/health-check.sh
./scripts/validate-db.sh
./scripts/validate-pipeline.sh
./scripts/test-config.sh

# Docker/Podman
cd compose && podman-compose up --build -d
podman-compose ps
podman-compose logs -f
podman-compose logs -f strategy
podman-compose down

# Tupã CLI (se instalado separadamente)
tupa --version
tupa check config/strategies/viper_smart_copy.tp
tupa codegen --backend hybrid --output /tmp/plan.json config/strategies/viper_smart_copy.tp

# Build
./scripts/build-tupa.sh
cargo build -p vipertrade-strategy --release
cargo run -p vipertrade-backtest -- --profile MEDIUM --start 2025-02-01 --end 2026-02-28

# Trading Commands (via API)
curl http://localhost:8080/api/v1/status
curl http://localhost:8080/api/v1/positions
curl http://localhost:8080/api/v1/trades?limit=20
curl http://localhost:8080/api/v1/leader/stats
curl -X POST http://localhost:8080/api/v1/system/kill-switch -d '{"confirm": true}'

# Database
podman exec -it vipertrade-postgres psql -U viper -d vipertrade

# SQL Queries Úteis
SELECT * FROM trades WHERE status = 'open';
SELECT SUM(pnl) as total_pnl FROM trades WHERE status = 'closed';
SELECT COUNT(*) as total_trades, COUNT(*) FILTER (WHERE pnl > 0) as winning_trades FROM trades WHERE status = 'closed';
SELECT * FROM position_snapshots WHERE reconciled = FALSE;
SELECT * FROM daily_metrics WHERE date = CURRENT_DATE;
SELECT * FROM tupa_audit_logs ORDER BY executed_at DESC LIMIT 10;
```

---

## 2️⃣0️⃣ **API Reference**

```yaml
# API Endpoints Web

GET /api/v1/portfolio
GET /api/v1/portfolio/equity?days=30
GET /api/v1/positions/open
POST /api/v1/positions/:id/close
GET /api/v1/trades?limit=50
GET /api/v1/performance/metrics
GET /api/v1/system/status
POST /api/v1/system/kill-switch
GET /api/v1/alerts?severity=&limit=

# Copy Trading Endpoints
GET /api/v1/copy/lead-traders
POST /api/v1/copy/start
POST /api/v1/copy/stop
GET /api/v1/copy/positions
GET /api/v1/copy/performance
GET /api/v1/leader/stats
POST /api/v1/leader/profile
POST /api/v1/leader/toggle

# WebSocket Events
portfolio:update
position:opened
position:closed
position:updated
trade:executed
system:status
alert:critical
alert:warning
```

---

## 2️⃣1️⃣ **Versionamento e Compatibilidade Tupã**

```yaml
versioning:
  vipertrade: "0.8.0"
  tupalang: 
    source: "crates.io"
    version: "0.8.0"
    license: "Apache-2.0/MIT"
  
  compatibility_matrix:
    vipertrade_0.8.0:
      tupa_min: "0.8.0"
      tupa_max: "0.8.x"
      rust_min: "1.75"
      podman_min: "4.0"
  
  update_policy:
    check_crates_io: weekly
    allow_patch_upgrades: true
    require_review_for_minor: true
    pin_exact_version_for_prod: true

environments:
  development:
    tupa_backend: "json"
    audit_level: "debug"
    seed: 42
    hot_reload: true
    
  testnet:
    tupa_backend: "json"
    audit_level: "detailed"
    seed: 42
    hot_reload: false
    
  production:
    tupa_backend: "llvm"
    audit_level: "compliance"
    seed: null
    hot_reload: false
```

---

## 2️⃣2️⃣ **Tupã crates.io Integration**

```yaml
tupa_crates_io:
  
  available_crates:
    - name: "tupa-runtime"
      version: "0.8.0"
      features: ["trading", "audit", "backtest"]
      purpose: "Pipeline execution engine"
    
    - name: "tupa-codegen"
      version: "0.8.0"
      purpose: "ExecutionPlan generation"
    
    - name: "tupa-audit"
      version: "0.8.0"
      purpose: "Structured audit logging"
    
    - name: "tupa-effects"
      version: "0.8.0"
      purpose: "Controlled side effects"
    
    - name: "tupa-parser"
      version: "0.8.0"
      purpose: "Source code parsing"
    
    - name: "tupa-typecheck"
      version: "0.8.0"
      purpose: "Static type checking"
  
  benefits_over_git:
    - "Faster builds (cargo registry cache)"
    - "Proper semver versioning"
    - "crates.io security audit integration"
    - "Easier upgrades (cargo update)"
    - "Offline builds with cargo vendor"
    - "Dual licensing (Apache-2.0/MIT)"
  
  installation:
    # Para embedding no ViperTrade:
    embedding:
      method: "cargo dependencies"
      Cargo_toml: |
        [dependencies]
        tupa-runtime = { version = "0.8.0", features = ["trading", "audit"] }
    
    # Para operações (CLI standalone):
    cli:
      method: "pre-built binary from GitHub Releases"
      install_command: |
        curl -L https://github.com/marciopaiva/tupalang/releases/latest/download/tupa-linux-x86_64 \
          -o /usr/local/bin/tupa
        chmod +x /usr/local/bin/tupa
      verify: "tupa --version"
  
  hybrid_distribution:
    embedding:
      benefits:
        - "Zero-copy integration"
        - "Type-safe pipeline execution"
        - "Compile-time validation"
    
    cli:
      benefits:
        - "No Rust toolchain required"
        - "Fast deployment"
        - "Stable ABI"
    
    ci_cd:
      build_strategy: "cache cargo registry"
      offline_build: "cargo vendor for air-gapped environments"
      artifact_signing: "cosign for release binaries"
  
  upgrade_policy:
    patch_upgrades:
      allowed: true
      auto: true
      example: "0.8.0 → 0.8.1"
    
    minor_upgrades:
      allowed: true
      auto: false
      requires: "manual review + testnet validation"
      example: "0.8.x → 0.9.0"
    
    major_upgrades:
      allowed: true
      auto: false
      requires: "full regression testing + migration guide"
      example: "0.x → 1.0"
```

---

## ✅ **Status Final**

| Seção | Status |
|-------|--------|
| 1-20 | ✅ Completo |
| 21. Versionamento Tupã | ✅ Completo |
| 22. Tupã crates.io Integration | ✅ Completo |
| **Total** | **100% ESPECIFICAÇÃO COMPLETA** |

