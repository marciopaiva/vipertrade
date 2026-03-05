# 🐍 **VIPERTRADE v0.0.1 - Especificação Técnica Completa**

> **Lead Trader Bot para Bybit Copy Trading Classic**  
> *Otimizado para Smart Copy Mode com Trailing Stop Dinâmico*

**Versão:** 0.0.1  
**Última Atualização:** Fevereiro 2026  
**Status:** ✅ 100% Especificado - Pronto para Implementação  
**Pares de Trading:** DOGEUSDT, XRPUSDT, TRXUSDT, XLMUSDT  
**Modo de Copy:** Smart Copy (Recomendado)  
**Trailing Stop:** Dinâmico Progressivo (Ratcheting)

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
12. Tupã Language Integration
13. Lead Trader Operations
14. Backtesting Engine
15. Smart Copy Mode Optimization
16. Dynamic Trailing Stop
17. Blocos de Desenvolvimento
18. Checklist de Validação
19. Comandos e Scripts Úteis
20. API Reference
```

---

## 1️⃣ **Visão Geral do Projeto**

### **1.1 Objetivo**

ViperTrade é um **Lead Trader Bot** automatizado para a plataforma **Bybit Copy Trading Classic**. Ele executa estratégia própria de trading e permite que outros usuários da Bybit copiem suas operações automaticamente via Smart Copy Mode.

### **1.2 Diferenciais Competitivos**

| Diferencial | Descrição | Benefício |
|------------|-----------|-----------|
| **Tupã Engine** | Linguagem de orquestração determinística | Auditabilidade completa, decisões reproduzíveis |
| **Trailing Stop Dinâmico** | Ajusta progressivamente conforme lucro aumenta | Protege ganhos, deixa winners correrem |
| **Smart Copy Optimized** | Position sizing previsível e estável | Menos falhas de copy, mais followers |
| **3 Perfis de Risco** | Conservative / Medium / Aggressive | Adaptável a diferentes condições de mercado |
| **Risk Management Multi-Camada** | 4 níveis de proteção | Preservação de capital em primeiro lugar |

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
│                    VIPERTRADE v0.0.1                            │
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

### **3.2 Parâmetros Específicos por Par**

```yaml
# config/trading/pairs.yaml
DOGEUSDT:
  price_precision: 5
  qty_precision: 0
  tick_size: 0.00001
  step_size: 1
  risk:
    stop_loss_pct: 0.02
    take_profit_pct: 0.04
    max_position_usdt: 15
    atr_multiplier: 0.5

XRPUSDT:
  price_precision: 4
  qty_precision: 1
  tick_size: 0.0001
  step_size: 1
  risk:
    stop_loss_pct: 0.018
    take_profit_pct: 0.036
    max_position_usdt: 20
    atr_multiplier: 0.6

TRXUSDT:
  price_precision: 5
  qty_precision: 0
  tick_size: 0.00001
  step_size: 1
  risk:
    stop_loss_pct: 0.015
    take_profit_pct: 0.030
    max_position_usdt: 20
    atr_multiplier: 0.7

XLMUSDT:
  price_precision: 5
  qty_precision: 0
  tick_size: 0.00001
  step_size: 1
  risk:
    stop_loss_pct: 0.018
    take_profit_pct: 0.036
    max_position_usdt: 20
    atr_multiplier: 0.6
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

### **4.2 Circuit Breakers**

```yaml
circuit_breakers:
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

### **4.3 Position Sizing (ATR-Adjusted)**

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

### **5.1 Tabelas Principais**

```sql
-- Tabela 1: trades (Histórico imutável)
CREATE TABLE trades (
    trade_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    bybit_order_id TEXT UNIQUE,
    symbol TEXT NOT NULL,
    side TEXT NOT NULL CHECK (side IN ('Long', 'Short')),
    quantity NUMERIC NOT NULL,
    entry_price NUMERIC NOT NULL,
    exit_price NUMERIC,
    leverage NUMERIC NOT NULL DEFAULT 2,
    pnl NUMERIC,
    pnl_pct NUMERIC,
    fees NUMERIC DEFAULT 0,
    funding_paid NUMERIC DEFAULT 0,
    opened_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    closed_at TIMESTAMPTZ,
    status TEXT NOT NULL DEFAULT 'open',
    close_reason TEXT,
    pipeline_version TEXT,
    decision_hash TEXT,
    paper_trade BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Tabela 2: position_snapshots (Reconciliação)
CREATE TABLE position_snapshots (
    snapshot_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    symbol TEXT NOT NULL,
    bybit_data JSONB NOT NULL,
    local_calculation JSONB NOT NULL,
    divergence NUMERIC,
    divergence_pct NUMERIC,
    reconciled BOOLEAN DEFAULT FALSE,
    snapshot_at TIMESTAMPTZ DEFAULT NOW(),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Tabela 3: system_events (Audit trail)
CREATE TABLE system_events (
    event_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_type TEXT NOT NULL,
    severity TEXT NOT NULL DEFAULT 'info',
    category TEXT,
    data JSONB NOT NULL,
    symbol TEXT,
    trade_id UUID REFERENCES trades(trade_id),
    timestamp TIMESTAMPTZ DEFAULT NOW(),
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Tabela 4: daily_metrics (Agregados)
CREATE TABLE daily_metrics (
    date DATE PRIMARY KEY,
    total_trades INTEGER DEFAULT 0,
    winning_trades INTEGER DEFAULT 0,
    losing_trades INTEGER DEFAULT 0,
    win_rate NUMERIC DEFAULT 0,
    total_pnl NUMERIC DEFAULT 0,
    total_pnl_pct NUMERIC DEFAULT 0,
    total_fees NUMERIC DEFAULT 0,
    max_drawdown NUMERIC DEFAULT 0,
    by_pair JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Tabela 5: profile_history (Mudanças de perfil)
CREATE TABLE profile_history (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    profile_code TEXT NOT NULL,
    changed_at TIMESTAMPTZ DEFAULT NOW(),
    reason TEXT,
    previous_profile TEXT,
    user_id TEXT
);

-- Tabela 6: schema_migrations (Versionamento)
CREATE TABLE schema_migrations (
    version TEXT PRIMARY KEY,
    applied_at TIMESTAMPTZ DEFAULT NOW()
);

-- Índices de performance
CREATE INDEX idx_trades_status ON trades(status);
CREATE INDEX idx_trades_symbol ON trades(symbol);
CREATE INDEX idx_trades_opened_at ON trades(opened_at);
CREATE INDEX idx_trades_pnl ON trades(pnl);
CREATE INDEX idx_snapshots_reconciled ON position_snapshots(reconciled);
CREATE INDEX idx_events_severity ON system_events(severity);
CREATE INDEX idx_events_timestamp ON system_events(timestamp);
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
  library: "governor"  # Rust crate
  strategy: "token_bucket"
  
  service_limits:
    market-data:
      websocket: "no_limit"
      rest_fallback: "100 requests / minute"
    executor:
      order_create: "10 orders / second"  # 50% do limite
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

### **7.1 Categorias de Erro**

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

### **9.1 Classificação de Incidentes**

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

### **10.1 Estrutura de Arquivos**

```bash
compose/
├── .env.example          # ✅ Pode commitar (template)
├── .env                  # ❌ NUNCA commitar (valores reais)
└── docker-compose.yml

secrets/
└── .gitkeep              # ✅ Pode commitar

.gitignore:
  **/.env
  **/.env.*
  !/.env.example
  secrets/*.key
```

### **10.2 Permissões de Segurança**

```bash
# Scripts de segurança
chmod 600 compose/.env
chmod 700 secrets/
chmod +x scripts/*.sh

# Verificação
./scripts/security-check.sh
```

### **10.3 Rotação de API Keys**

```yaml
api_key_rotation:
  frequency_days: 90
  alert_before_days: 7
  procedure:
    - "Criar nova API key na Bybit"
    - "Testar nova key em testnet"
    - "Atualizar compose/.env"
    - "Restart serviços"
    - "Validar por 1 hora"
    - "Revogar key antiga"
    - "Documentar rotação"
  
  bybit_api_permissions:
    - "Order: Enable"
    - "Position: Enable"
    - "Account: Read-only"
    - "Withdraw: DISABLED (CRÍTICO)"
  
  ip_whitelist:
    enabled: true
    add_server_ip: true
    update_on_ip_change: true
  
  two_factor_auth:
    required: true
    enabled_on_account: true
```

### **10.4 Security Audit Checklist**

```bash
pre_mainnet_checklist:
  - [ ] API keys com permissões mínimas
  - [ ] Withdraw permission DESATIVADA
  - [ ] IP whitelist configurado
  - [ ] 2FA ativado na conta Bybit
  - [ ] .env com permissão 600
  - [ ] .env NÃO está no Git
  - [ ] Scripts de security-check passam
  - [ ] Kill switch testado
  - [ ] Emergency procedures documentados
  - [ ] Backup de database configurado
  - [ ] Alertas críticos testados
  - [ ] Paper trading 21 dias estável
  - [ ] Backtest aprovado em stress scenarios

code_audit:
  - [ ] No secrets hardcoded
  - [ ] All inputs validated
  - [ ] Error messages don't leak info
  - [ ] Rate limiting implemented
  - [ ] SQL injection protected
  - [ ] XSS protected (web)
  - [ ] CSRF protected (web)
  - [ ] Dependencies atualizadas (cargo audit)
```

---

## 1️⃣1️⃣ **Discord Notifications**

### **11.1 Configuração de Webhooks**

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
```

### **11.2 Templates de Alertas**

```yaml
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
```

### **11.3 Alert Recipients Clarification**

```yaml
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

## 1️⃣2️⃣ **Tupã Language Integration**

### **12.1 Visão Geral**

```yaml
tupa_integration:
  
  usage_model: "embedded_library"
  
  components:
    - "tupa-runtime"      # Execução de pipelines
    - "tupa-codegen"      # Geração de ExecutionPlan
    - "tupa-validator"    # Validação de constraints
  
  pipeline_files:
    - "config/strategies/viper_smart_copy.tp"
    - "config/strategies/viper_conservative.tp"
    - "config/strategies/viper_medium.tp"
    - "config/strategies/viper_aggressive.tp"
  
  integration_point: "services/strategy/src/tupa_engine.rs"
  
  cargo_dependencies:
    - "tupa-runtime = { path = \"../tupalang/crates/tupa-runtime\" }"
    - "tupa-codegen = { path = \"../tupalang/crates/tupa-codegen\" }"
    - "tupa-validator = { path = \"../tupalang/crates/tupa-validator\" }"
```

### **12.2 Pipeline Tupã para Smart Copy**

```tupã
// config/strategies/viper_smart_copy.tp

type MarketSignal {
  symbol: string,
  current_price: f64,
  atr_14: f64,
  volume_24h: f64,
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
}

extern fn viper::calculate_smart_copy_size(signal: MarketSignal, config: SmartCopyConfig, last_position_size: f64): f64;
extern fn viper::check_smart_copy_constraints(size: f64, config: SmartCopyConfig): bool;
extern fn viper::validate_entry_conditions(signal: MarketSignal, config: SmartCopyConfig): bool;

pipeline ViperSmartCopy @deterministic(seed=42) {
  input: MarketSignal,
  
  constraints: [
    { metric: "win_rate_30d", ge: 0.50 },
    { metric: "max_drawdown_pct", le: 0.10 },
    { metric: "avg_risk_reward", ge: 2.0 },
    { metric: "copy_success_rate", ge: 0.95 },
    { metric: "avg_slippage_pct", le: 0.01 },
  ],
  
  steps: [
    step("validate_entry") {
      viper::validate_entry_conditions(input, config)
    },
    
    step("calc_smart_size") {
      viper::calculate_smart_copy_size(input, config, last_position_size)
    },
    
    step("validate_size") {
      viper::check_smart_copy_constraints(calc_smart_size, config)
    },
    
    step("decision") {
      if validate_entry && validate_size && calc_smart_size >= config.min_position_usdt {
        let side = if input.trend_score > 0 { "Long" } else { "Short" };
        {
          action: if side == "Long" { "ENTER_LONG" } else { "ENTER_SHORT" },
          symbol: input.symbol,
          quantity: calc_smart_size,
          leverage: config.max_leverage,
          entry_price: input.current_price,
          stop_loss: if side == "Long" { input.current_price * (1 - config.stop_loss_pct) } else { input.current_price * (1 + config.stop_loss_pct) },
          take_profit: if side == "Long" { input.current_price * (1 + config.take_profit_pct) } else { input.current_price * (1 - config.take_profit_pct) },
          reason: "smart_copy_optimized",
          smart_copy_compatible: true
        }
      } else {
        { action: "HOLD", quantity: 0, smart_copy_compatible: false }
      }
    }
  ],
  
  validation: {
    assert(decision.action == "HOLD" || decision.smart_copy_compatible == true);
    assert(decision.action == "HOLD" || (decision.quantity * decision.entry_price >= config.min_position_usdt && decision.quantity * decision.entry_price <= config.max_position_usdt));
  },
  
  output: {
    decision: decision,
    execution_hash: hash(decision),
    timestamp: now(),
    pipeline_version: "0.8.1",
    smart_copy_mode: true
  }
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
    cumulative_order_value_per_pair: 300000  # USDT per follower per pair
    calculation: "Sum of (order_cost × leverage) for all open positions"
  
  our_strategy:
    target_follower_profile:
      typical_investment: 100-1000  # USDT
      typical_leverage: 5-10x
      implied_max_order_cost: 100-200  # USDT per trade
    
    our_max_position_usdt: 200  # Para não excluir followers pequenos
    rationale: "Larger positions still copied by larger followers; smaller positions accessible to all"
```

---

## 1️⃣4️⃣ **Backtesting Engine**

### **14.1 Arquitetura**

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

### **15.1 Por Que Smart Copy**

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
    follower_position: 50  # 10 × (500/100) × 1.0
```

### **15.2 Configurações para Smart Copy**

```yaml
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

### **15.3 Guia para Followers**

```markdown
# Como Copiar o ViperTrade

## Configuração Recomendada
- Copy Mode: Smart Copy
- Copy Ratio: 1.0 (padrão)
- PCSL: 20-25%
- Trailing Stop: 20% (opcional)
- Max Slippage: 1.5%

## Por Que Smart Copy?
✅ Sizing Automático
✅ Leverage Alinhado
✅ Menos Falhas
✅ Ideal para Iniciantes

## PCSL Recomendado: 20-25%
ViperTrade já tem stop loss por trade (1.5-2.5%). PCSL é proteção extra a nível de portfolio.
```

---

## 1️⃣6️⃣ **Dynamic Trailing Stop**

### **16.1 Visão Geral**

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
```

### **16.2 Configuração por Perfil**

```yaml
trailing_stop_by_profile:
  
  CONSERVATIVE:
    activate_after_profit_pct: 0.01      # 1% já ativa
    initial_trail_pct: 0.005             # 0.5% inicial (apertado)
    ratchet_levels:
      - at_profit_pct: 0.02
        trail_pct: 0.008
      - at_profit_pct: 0.04
        trail_pct: 0.012
      - at_profit_pct: 0.06
        trail_pct: 0.02
    move_to_break_even_at: 0.015
  
  MEDIUM:
    activate_after_profit_pct: 0.015     # 1.5% ativa
    initial_trail_pct: 0.008             # 0.8% inicial
    ratchet_levels:
      - at_profit_pct: 0.03
        trail_pct: 0.012
      - at_profit_pct: 0.06
        trail_pct: 0.02
      - at_profit_pct: 0.10
        trail_pct: 0.035
    move_to_break_even_at: 0.02
  
  AGGRESSIVE:
    activate_after_profit_pct: 0.02      # 2% ativa
    initial_trail_pct: 0.01              # 1% inicial (espaço)
    ratchet_levels:
      - at_profit_pct: 0.05
        trail_pct: 0.02
      - at_profit_pct: 0.10
        trail_pct: 0.035
      - at_profit_pct: 0.15
        trail_pct: 0.05
    move_to_break_even_at: 0.03
```

### **16.3 Mecânica de Ratcheting**

```yaml
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
```

### **16.4 Implementação Técnica**

```yaml
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
## Bloco 1: Setup do Projeto (Dia 1-2)
- [ ] Criar estrutura de diretórios
- [ ] Inicializar Git repository
- [ ] Criar .gitignore
- [ ] Criar README.md
- [ ] Criar compose/.env.example
- [ ] Criar scripts/init-secrets.sh
- [ ] Criar scripts/security-check.sh
- [ ] Testar init-secrets.sh

## Bloco 2: Docker Compose (Dia 2-3)
- [ ] Criar compose/docker-compose.yml
- [ ] Configurar serviço postgres
- [ ] Configurar serviço redis
- [ ] Configurar rede vipertrade-net
- [ ] Configurar volumes persistentes
- [ ] Adicionar health checks
- [ ] Testar podman-compose up

## Bloco 3: Database Schema (Dia 3-4)
- [ ] Criar database/schema.sql
- [ ] Criar tabela trades
- [ ] Criar tabela position_snapshots
- [ ] Criar tabela system_events
- [ ] Criar tabela daily_metrics
- [ ] Criar índices de performance
- [ ] Testar schema no postgres

## Bloco 4: Configuração dos Pares (Dia 4-5)
- [ ] Criar config/trading/pairs.yaml
- [ ] Configurar DOGEUSDT
- [ ] Configurar XRPUSDT
- [ ] Configurar TRXUSDT
- [ ] Configurar XLMUSDT
- [ ] Configurar global_settings

## Bloco 5: Market Data Service (Dia 5-8)
- [ ] Criar services/market-data/
- [ ] Implementar WebSocket client Bybit
- [ ] Subscrever canais públicos (4 pares)
- [ ] Normalizar dados recebidos
- [ ] Publicar no Redis Pub/Sub
- [ ] Implementar reconnect strategy
- [ ] Criar Dockerfile

## Bloco 6: Strategy Engine (Dia 8-12)
- [ ] Criar services/strategy/
- [ ] Implementar Tupã Engine integration
- [ ] Criar Risk Manager module
- [ ] Implementar position sizing (ATR)
- [ ] Implementar Dynamic Trailing Stop
- [ ] Implementar circuit breaker
- [ ] Criar Dockerfile

## Bloco 7: Order Executor (Dia 12-16)
- [ ] Criar services/executor/
- [ ] Implementar Bybit REST API client
- [ ] Suporte a OCO orders
- [ ] Implementar retry logic
- [ ] Rate limiting
- [ ] Reconciliation
- [ ] Criar Dockerfile

## Bloco 8: Monitor Service (Dia 16-20)
- [ ] Criar services/monitor/
- [ ] Implementar health checks
- [ ] Implementar Discord webhook client
- [ ] Implementar reconciliation engine
- [ ] Implementar Trailing Stop Monitor
- [ ] Criar Dockerfile

## Bloco 9: Error Handling (Dia 20-22)
- [ ] Implementar error handling matrix
- [ ] Criar error types estruturados
- [ ] Implementar backoff strategies
- [ ] Testar cada tipo de erro

## Bloco 10: Testing & Validation (Dia 22-28)
- [ ] Configurar testnet environment
- [ ] Rodar paper trading 14 dias
- [ ] Backtest com dados históricos
- [ ] Validar risk management
- [ ] Documentar resultados

## Bloco 11: Documentation (Dia 28-30)
- [ ] Escrever ARCHITECTURE.md
- [ ] Escrever OPERATIONS.md
- [ ] Documentar procedimentos de emergência
- [ ] Atualizar README.md

## Bloco 12: Mainnet Micro Deployment (Dia 30+)
- [ ] Criar API keys mainnet
- [ ] Deploy em produção
- [ ] Monitoramento intensivo 72h
- [ ] Ajustar parâmetros se necessário

## Bloco 13: Web Dashboard (Dia 30-40)
- [ ] Criar estrutura web/
- [ ] Configurar Next.js 14
- [ ] Implementar páginas principais
- [ ] Integrar WebSocket real-time
- [ ] Criar Dockerfile

## Bloco 14: Smart Copy Optimization (Dia 40-43)
- [ ] Configurar smart_copy_optimized.yaml
- [ ] Implementar position smoothing
- [ ] Testar com followers de teste
- [ ] Documentar para followers

## Bloco 15: Dynamic Trailing Stop (Dia 43-45)
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

# Docker/Podman
cd compose && podman-compose up --build -d
podman-compose ps
podman-compose logs -f
podman-compose down

# Backtesting
./scripts/run-backtest.sh MEDIUM 2025-02-01 2026-02-28

# Trading Commands
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
