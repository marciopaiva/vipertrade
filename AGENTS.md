# ViperTrade — Análise Completa do Negócio

## Visão Geral

ViperTrade é um **Lead Trader Bot para Bybit Copy Trading Classic**. Consome dados de 3 exchanges (Bybit, Binance, OKX), aplica estratégia algorítmica via TupaLang DSL, gera decisões de trade, e executa na Bybit em modo paper/testnet/mainnet.

**Stack:** Rust 1.83 + PostgreSQL 16 + Redis 7 + TupaLang 0.11 + Next.js + Docker/K8s (Kind)

## Arquitetura de Serviços

### 1. viper-domain (crates/viper-domain)
- Contratos compartilhados: `MarketSignal` (~48 campos), `StrategyDecision`, eventos com envelope (UUID + timestamp + schema v1.0)
- `TradingMode` enum (Paper/Testnet/Mainnet), helpers de config (DB, Redis, Bybit, trading pairs)
- 4 Redis streams: `viper:market_data`, `viper:decisions`, `viper:executor_events`, `viper:control_events`. Consumer groups: `strategy`, `executor`, `ws-bridge`

### 2. viper-market-data (services/market-data) — 1114+668+339+399 linhas
- A cada 5s: fetch 220 candles 1min de Bybit+Binance+OKX → alinha timestamps → computa indicadores (ATR, ADX, RSI, MACD, Bollinger, EMA, Volume Ratio) → weighted avg entre exchanges → side latch (2 ciclos debounce) → BTC context injection → publica no Redis
- Trend score composto: EMA(20/50) 40% + RSI 25% + MACD 25% × volume multiplier
- **Fragilidade**: Exige TODAS as 3 exchanges — se uma falha, pula o símbolo inteiro

### 3. viper-analytics (services/analytics) — 712 linhas
- A cada 5s: busca 200 candles, computa RSI(14) → trend_score [-1,+1], salva no PostgreSQL
- Query SQL com LATERAL JOIN pareia sinal com preço futuro, calcula hit rate por exchange/symbol
- Endpoint: `GET /scores` (cache em memória)
- **Redundância**: Re-fetch 200 candles inteiros a cada ciclo — cache incremental reduziria drasticamente banda

### 4. viper-strategy (services/strategy) — 4889+310+272+577+135+409+84 linhas
- Motor de decisão TupaLang. Pipeline ViperSmartCopy (14 steps): daily loss → consecutive losses → validate_entry (17 checks) → funding → smart size → trailing config → decision → cooldown → thesis → audit
- Trailing stop com ratchet levels, break-even lock, AI tightening (só afrouxa)
- Thesis invalidation: escore -100 a +100 com 14 componentes, ladder 4 estágios, 2 ticks confirmação
- Portfolio selection: fila de 3s, flush a cada 500ms por rank
- **Grande demais**: lib.rs 4889 linhas mesmo após extração de 4 módulos

### 5. viper-executor (services/executor) — 3518 linhas (single file)
- Subscribe Redis → deserialize → dedup (DB + memória) → runtime controls → paper path (DB) ou live path (Bybit REST + HMAC-SHA256)
- Paper mode: sem slippage nem partial fills — superestima performance
- Reconciliação a cada 60s (drift > $5)
- **Monolítico**: 3518 linhas sem módulos internos

### 6. viper-monitor (services/monitor) — 731 linhas
- A cada 5min: compara posições DB vs Bybit, classifica drift (info/warning/error/critical), envia Discord + Redis
- **Cooldown de 5min**: se drift persiste, só 1 alerta; deveria escalar severidade

### 7. viper-api (services/api) — 2933+374+92+215+214 linhas
- 16 GET + 3 POST + 1 WS. Rate limit: public/data 100 req/min, control 20 req/min (per-IP sliding window)
- Auth: token-based só nos POSTs de controle. WS sem autenticação.
- WS bridge Redis→browser com DLQ 2000 mensagens para lag recovery
- **Rate limiter in-memory**: não escala horizontalmente. WS sem auth = dados expostos.

### 8. viper-ai-analyst (services/ai-analyst) — 2626+653+282 linhas
- Diagnósticos rule-based (sem LLM): exit pressure, directional bias, entry pressure, thesis quality, symbol risk
- Gera ExecutionAdvice (constructive/selective/defensive/observation) e ActivePositionAdvice
- Tuning grid: 16 variantes determinísticas, sorted por delta_net_pnl
- `POST /analyze/recent`, `POST /sweep`, `POST /analyze/tuning`

### 9. viper (services/viper) — binário unificado
- VIPER_ROLE env var → dispatch para o serviço correspondente
- Docker multi-stage (rust builder + python runtime)

### 10. web (services/web) — Next.js App Router
- Dashboard com Zustand + Tailwind + WebSocket

## Problemas Identificados

### 🔴 Críticos
1. Market-data all-or-nothing: 3 exchanges exigidas → ~3% ciclos parciais
2. Strategy lib.rs 4889 linhas
3. Executor paper mode sem slippage/fills parciais
4. Executor 3518 linhas sem módulos

### 🟡 Altos
5. Rate limiter in-memory (não escala horizontalmente)
6. WebSocket sem autenticação
7. Redis pub/sub sem backpressure
8. Sem TLS entre serviços
9. Analytics re-fetch 200 candles a cada ciclo

### 🟢 Médios
10. TupaLang indisponível em dev (cargo-tupa não compilado)
11. Kill-switch sem confirmação do executor
12. tracing_subscriber init() em cada run() — panica se 2 serviços no mesmo processo
13. Reconciliação detect-only por padrão
15. selection_window_ms de 3s muito longo

### 🔵 Baixos
16. Versão workspace 0.8.2 vs packages 0.9.0
17. viper_api::run sem Result
18. DLQ 2000 ≠ broadcast 8192
19. Componentes de scoring com peso 5 têm baixo impacto
20. ai-analyst limit=12 arbitrário

## Plano de Implementação (28-Jun-2026)

### Legenda
**Esforço:** 🟢 < 2h | 🟡 2-8h | 🔴 8-24h | ⚫ > 24h
**Impacto:** 🔥 crítico | ⚡ alto | 💫 médio | 💡 baixo
**Depois de:** itens que devem ser concluídos antes

---

### Fase 1 — Quick Wins (limpeza técnica)
Itens isolados, baixo risco, efeito imediato.

| # | Item | Esforço | Depois de | Entregas |
|---|------|---------|-----------|----------|
| 16 | Unificar versão workspace | 🟢 | — | Root `Cargo.toml` version → `0.9.0`. `cargo build` + `cargo test` |
| 17 | Padronizar `viper_api::run` com `Result` | 🟢 | — | Mudar signature para `Result<(), Box<dyn Error>>`. Ajustar `viper/main.rs`. |
| 18 | Alinhar DLQ capacity com broadcast buffer | 🟢 | — | Mudar `DLQ_CAPACITY` para `8192`. Verificar `VecDeque::with_capacity`. |
| 20 | Tornar ai-analyst limit configurável | 🟢 | — | Env var `AI_ANALYST_SYMBOL_LIMIT` (default 12). |
| 12 | `try_init()` em vez de `init()` nos serviços | 🟢 | — | Substituir `tracing_subscriber::fmt().init()` por `.try_init()` em todos os 7 `run()`. |
| 10 | Script setup-dev.sh (TupaLang) | 🟢 | — | `scripts/setup-dev.sh`: compila `cargo-tupa` do workspace irmão `../tupalang/` e instala no PATH (já compilado e instalado). |

**Total Fase 1:** ~5h 🟢. 6 PRs independentes, podem ser paralelizados.

---

### Fase 2 — Resiliência do Pipeline de Dados
Itens que aumentam robustez do fluxo market-data → strategy → executor.

| # | Item | Esforço | Depois de | Entregas |
|---|------|---------|-----------|----------|
| 1 | Market-data: suportar N/3 exchanges | 🔴 | — | `min_exchanges_required` configurável (default 2). Se uma exchange falha, degradar gracefulmente. Ajustar `aggregate_signals` para N exchanges. Atualizar `consensus_ratio`, `exchanges_available`. Testes com 2/3 exchanges. |
| 9 | Analytics: cache incremental de candles | 🟡 | — | Em vez de re-fetch 200 candles a cada 5s, fetch incremental (último candle). Manter array em memória, append + shift. Economia de ~90% de banda de API. |
| 15 | Reduzir `selection_window_ms` | 🟢 | 1 | Default 3000 → 1000. Validar impacto em portfolio selection. |

**Total Fase 2:** ~20h 🟡🔴. Item 1 é o maior investimento da fase (~12h).

---

### Fase 3 — Segurança
Itens que fecham vetores de ataque.

| # | Item | Esforço | Depois de | Entregas |
|---|------|---------|-----------|----------|
| 6 | WebSocket com autenticação | 🟡 | — | JWT token no handshake WS. Verificar token no `ws_client()`. Renew token periódico. Cliente web envia token no query param ou subprotocol. |
| 5 | Rate limiter centralizado (Redis) | 🟡 | — | Substituir `HashMap<String, SlidingWindow>` in-memory por Redis sorted sets (ZADD + ZREMRANGEBYSCORE + ZCOUNT). Sliding window server-side. |
| 8 | TLS entre serviços internos | ⚫ | 6 | Certificados auto-assinados ou mTLS via service mesh (Linkerd). Redis TLS, PostgreSQL TLS, HTTP → HTTPS. **Pode ser postergado** se operar apenas em K8s isolado. |

**Total Fase 3:** ~40h 🟡⚫. Item 8 é cross-cutting e pode ser adiado.

---

### Fase 4 — Refatoração Strategy
Reduzir dívida técnica do núcleo de decisão.

| # | Item | Esforço | Depois de | Entregas |
|---|------|---------|-----------|----------|
| 2 | Quebrar strategy lib.rs em módulos de step | 🔴 | — | Extrair cada step function para módulo dedicado: `daily_loss.rs`, `consecutive_losses.rs`, `validate_entry.rs`, `funding.rs`, `smart_size.rs`, `trailing_config.rs`, `decision.rs`, `cooldown.rs`, `signal_confirmation.rs`, `thesis_confirmation.rs`, `audit.rs`. lib.rs vira orchestrator (~300 linhas). |
| 19 | Revisar pesos de scoring (componentes com peso 5) | 💫 | 2 | Avaliar impacto dos componentes com peso 5 (bollinger_bandwidth, macd_histogram, volume_ratio). Se impacto < 1%, considerar remover ou agrupar. |

**Total Fase 4:** ~24h 🔴. Item 2 é o maior refactor do plano.

---

### Fase 5 — Executor
Modularizar e adicionar realismo ao paper mode.

| # | Item | Esforço | Depois de | Entregas |
|---|------|---------|-----------|----------|
| 4 | Modularizar executor (3518→dir) | ⚫ | — | Quebrar `lib.rs` em: `orders.rs` (submit + fetch meta), `reconciliation.rs` (reconcile tick), `bybit_client.rs` (HMAC + HTTP), `state.rs` (ExecutorState, dedup set), `risk.rs` (symbol constraints, normalização), `paper.rs` (simulação paper), `run.rs` (event loop + init). |
| 3 | Slippage model no paper mode | 🟡 | 4 | Adicionar slippage estocástico (±0.03-0.08%) e fill probability (95-99%) no paper mode. Ajustar `persist_trade()` para usar preço com slippage. |
| 11 | Kill-switch com confirmação do executor | 🟡 | — | Quando API escreve `api_kill_switch_set` no DB, executor deve publicar ack no Redis `viper:executor_events`. API espera ack (timeout 5s) antes de retornar sucesso. |

**Total Fase 5:** ~40h ⚫🔴. Item 4 é o maior de todo o plano (~24h).

---

### Fase 6 — Monitoramento e Operações
Itens que melhoram observabilidade e resposta a incidentes.

| # | Item | Esforço | Depois de | Entregas |
|---|------|---------|-----------|----------|
| 7 | ~~Redis streams em vez de pub/sub~~ | ~~🔴~~ | ~~1~~ | ~~Concluído~~ |
| 13 | ~~Ativar auto-fix na reconciliação (live mode)~~ | ~~🟡~~ | ~~5~~ | ~~Concluído~~ |
| 14 | ~~Escalar severidade no cooldown Discord~~ | ~~🟡~~ | ~~—~~ | ~~Removido do escopo — Discord não será implementado no momento.~~ |

**Total Fase 6:** ~28h 🔴🟡. Items 7 e 13 concluídos.

---

### Mapa de Dependências

```
Fase 1 (Quick Wins) ──────────────────────────┬─────────────────────────────┐
                                               │                             │
Fase 2 (Pipeline Resiliência) ◄── 1 ──────────┤                             │
  │                                            │                             │
  ├── Item 15 precisa do Item 1 ───────────────┤                             │
  │                                            │                             │
Fase 3 (Segurança)                             │                             │
  ├── Item 8 precisa do Item 6 ────────────────┤                             │
  │                                            │                             │
Fase 4 (Strategy) ─────────────────────────────┴───── 19 precisa do 2 ──────┤
  │                                                                          │
Fase 5 (Executor) ────────────────────────────┬───── 3 precisa do 4 ────────┤
  │                                            │                             │
Fase 6 (Monitoramento) ◄── 13 precisa do 5 ───┤                             │
                                                                             │
Serviço                                        Responsável                  │
─────────────────────────────────────────────────────────────────────────────
viper-market-data    │ 1, 9                       ~22h                      │
viper-analytics      │ 9                                           ~6h      │
viper-strategy       │ 2, 15, 19                  ~28h                      │
viper-executor       │ 3, 4, 5, 7, 11, 13         ~64h ✅ 3,4,7,11,13                   │
viper-api            │ 5, 6, 7, 8                 ~24h ✅ 7                           │
viper-monitor        │ 7, 13                      ~8h   🔴 7, 13 (ambos concluídos)   │
viper-ai-analyst     │ 20                         ~1h                       │
viper                │ 17                         ~0.5h                     │
viper-domain         │ (sem items diretos)         —                        │
web                  │ 6 (WS client auth)          ~4h                       │
```

---

### Estratégia de Release

| Release | Fases | Itens | Estimativa | Riscos |
|---------|-------|-------|------------|--------|
| **v0.10.0** | Fase 1 | 10, 12, 16, 17, 18, 20 | ~5h 🟢 | Baixo |
| **v0.11.0** | Fase 2 | 1, 9, 15 | ~20h 🟡🔴 | Médio (muda lógica de consenso) |
| **v0.12.0** ✅ | Fase 3 | 5, 6 | ~12h 🟡 | Médio (auth breaking change) |
| **v0.13.0** ✅ | Fase 4 | 2, 19 | ~24h 🔴 | Alto (refactor grande) |
| **v0.14.0** ✅ | Fase 5 | 4 | ~24h ⚫ | Alto (refactor executor) |
| **v0.15.0** ✅ | Fase 5 | 3, 11 | ~12h 🟡 | Médio (slippage paper, kill-switch ack) |
| **v0.16.0** ✅ | Fase 6 | 7 | ~16h 🔴 | Alto (migra Redis) — concluído |
| **v0.17.0** ✅ | Fase 6 | 13 | ~8h 🟡 | Médio (auto-fix reconciliação live mode) — concluído |
| **v1.0.0-rc** | Fase 3 (TLS) | 8 | ~24h ⚫ | Médio (infra cross-cutting) |

## Estado Atual (28-Jun-2026)

### Concluído (Fase 1 + Fase 2 + Fase 3)
- **Fase 1 (Quick Wins) — 6/6 concluídos:**
  - Item 16: Versões unificadas (root + viper-domain → 0.9.0)
  - Item 17: `viper_api::run` com `Result<(), Box<dyn Error>>`
  - Item 18: DLQ capacity alinhado com broadcast buffer (2000→8192)
  - Item 20: `AI_ANALYST_SYMBOL_LIMIT` env var (default 12)
  - Item 12: `try_init()` em vez de `init()` nos 7 serviços
  - Item 10: `scripts/setup-dev.sh` (cargo-tupa do workspace irmão)
- **Fase 2 (Pipeline Resiliência) — 3/3 concluídos:**
  - Item 1: Market-data suporta N/3 exchanges via `MARKET_DATA_MIN_EXCHANGES` env var (default 2). Strategy relaxado de `>=3` para `>=2`.
  - Item 9: Analytics com cache incremental de candles (`CandleCache` com timestamps, merge dedup, bootstrap 200 → fetch 2). ~90% menos banda de API.
  - Item 15: `selection_window_ms` 3000→1000.
- **Fase 3 (Segurança) — 2/3 concluídos (Item 8 TLS postergado):**
  - Item 5: Rate limiter centralizado Redis (`RateLimiter` enum com backends `InMemory`/`Redis`, fail-open em erro Redis, sorted sets ZADD/ZREMRANGEBYSCORE/ZCOUNT + EXPIRE).
  - Item 6: WS auth JWT opcional (`API_JWT_SECRET` env var). `GET /api/v1/auth/ws-token` endpoint emite JWT 1h. `ws_auth_filter` com query param `?token=`. Frontend `WebSocketClient` com `refreshToken()` e `resolveUrl()` automático.
- Redis streams centralizados em viper-domain (Item 7)
- DLQ in-memory (VecDeque 8192) para lag recovery WS
- Teste E2E executor (Redis → DB)
- Paginação em /decisions, /events, /trades
- +14 testes em market-data (38→52)
- Strategy quebrado em 23 módulos (6152→2776 linhas)
- cargo-tupa compilado e instalado no PATH a partir do workspace irmão tupalang/

### Concluído (Fase 4 — Strategy Refactor)
- **Item 2 (Fase 4) — 23 módulos extraídos e integrados, lib.rs 6152→2776 linhas (-55%):**
  - **helpers.rs** (214 linhas): 22 funções utilitárias
  - **types.rs** (310 linhas): todos os structs compartilhados
  - **thesis.rs** (577 linhas): avaliação de tese (8 funções)
  - **config.rs** (754 linhas): `StrategyConfig` struct + impl completo. `pub use config::StrategyConfig;`
  - **trailing.rs** (396 linhas): trailing stop, ratchet, break-even, exit evaluation
  - **trailing_config.rs** (46 linhas): trailing config step
  - **filters.rs** (342 linhas): entry guard policy, temporal pipeline state
  - **db.rs** (207 linhas): fetch/update trades, hashing, audit log
  - **fetch.rs** (58 linhas): wallet & ai-analyst API clients
  - **validate_entry.rs** (347 linhas): entry validation, RSI/Bollinger/MACD quality scores
  - **decision.rs** (127 linhas): decision step and summary
  - **smart_size.rs** (91 linhas): smart position sizing
  - **funding.rs** (73 linhas): funding rate check
  - **validate_size.rs** (84 linhas): size validation step
  - **audit.rs** (27 linhas): pre-store audit
  - **ai_advice.rs** (206 linhas): execution advice veto/sizing/quarantine
  - **daily_loss.rs** (18 linhas): daily loss constraint metrics
  - **consecutive_losses.rs** (18 linhas): consecutive loss constraint metrics
  - **equity_floor.rs** (6 linhas): equity floor constraint step
  - **signal_confirmation.rs** (6 linhas): signal confirmation step
  - **thesis_confirmation.rs** (6 linhas): thesis confirmation step
  - **cooldown.rs** (6 linhas): cooldown guard step
  - lib.rs reduzido de 6152→**2776** linhas (-3376, ~55%)
  - `cargo check --workspace` + `cargo test` (31/31) passam, sem erros, sem warnings
  - Etapas: declaração em batch, remoção Python bottom-up, fix visibilidade `pub(crate)`, limpeza de imports órfãos

### Concluído
- **Item 19 (Fase 4) — Revisar pesos de scoring (componentes com peso 5)**: 3 componentes weight-5 em Entry Policy simplificados: `bollinger_bandwidth` mergeado em `bollinger_extension` (w=8→13), `macd_histogram` mergeado em `macd_cross` (w=10→15), `volume_ratio` mantido (único). 14→12 componentes. Impacto <1% no score clampado. `cargo check` + 31/31 testes ok.

### Concluído (Fase 5 — Executor)
- **Item 4 (Fase 5) — Modularizar executor (3518→dir)**: lib.rs quebrado em 5 módulos extraídos + lib.rs reduzido de 3518→1964 linhas (-44%). Produção: ~2473→1044 linhas (-58%).
  - **orders.rs** (741 linhas): `submit_market_order`, `close_open_trade`, `persist_bybit_fills`, `fetch_order_execution_*`, `normalize_order_quantity`, `ensure_min_notional`, `set_bybit_trailing_stop`, formatação/arredondamento
  - **state.rs** (317 linhas): `ExecutorState`, `persist_trade`, `mark_processed`, `claim_processed_event`, `upsert_decision_audit`, `fetch_runtime_controls`, `count_open_trades`
  - **bybit_client.rs** (227 linhas): `bybit_sign`, `bybit_public_get`/`bybit_private_get`/`bybit_private_post`, `parse_bybit_json_response`, `run_bybit_sanity_checks`
  - **reconciliation.rs** (210 linhas): `run_reconciliation_tick`, `record_reconciliation_event`, `apply_reconciliation_reduce_local`, `local_open_qty`, `reconciliation_event_meta`
  - **risk.rs** (96 linhas): `fetch_symbol_constraints`, `get_symbol_constraints`, `parse_positive_f64`
  - `cargo check --workspace` + `cargo test` (29/29) passam, sem erros
  - Paper mode functions mantidos em lib.rs (são funções de teste com `#[tokio::test]`)
  - Etapas: análise bottom-up com Python, extração de blocos via brace counting, fix visibilidade `pub(crate)`

### Concluído (Fase 5 — Executor continuação)
- **Item 3 — Slippage model no paper mode**: `paper_adverse_slippage()` aplica slippage estocástico adverso (0.03-0.08%, direção dependente da ação) em entradas e saídas paper. `paper_fill_check()` com 97% fill probability — ordens não preenchidas viram `paper_not_filled`. Dependência `rand = "0.8"` adicionada. `cargo check --workspace` + testes (172/172) ok.
- **Item 11 — Kill-switch com confirmação do executor**: API publica `kill_switch_sync` na stream `viper:control_events` com `request_id` UUID. Executor XREADGROUP em task separada → lê DB → publica ack `kill_switch_ack` em `viper:executor_events`. API espera ack com timeout 5s via XREAD; `KillSwitchResponse.acknowledged` reflete resultado. `viper:control_events` registrado em `viper-domain`. Zero warnings.

### Concluído (Fase 6 — Redis Streams)
- **Item 7 — Redis streams**: Todos os serviços migrados de `PUBLISH`/`SUBSCRIBE` para `XADD`/`XREADGROUP` com consumer groups. ~10k MAXLEN trim. `monitor` ainda usa pub/sub (fora do escopo — único consumidor, sem ganho com streams).
  - **viper-domain**: constantes `REDIS_STREAM_*`, `STREAM_GROUP_*`, helpers `stream_publish()` (XADD MAXLEN ~10k) e `stream_ensure_group()` (XGROUP CREATE $ MKSTREAM). `redis` adicionado como dependência.
  - **market-data**: `conn.publish(REDIS_CHANNEL_MARKET_DATA)` → `stream_publish(REDIS_STREAM_MARKET_DATA)`. 0 warnings.
  - **strategy**: pubsub + `on_message` → `mpsc::unbounded_channel` + background task XREADGROUP + XACK. `publish_decision_event` → `stream_publish(REDIS_STREAM_DECISIONS)`. 10 pre-existing warnings (unrelated).
  - **executor**: dois pubsub subscribers (decisions + control_events) → duas tasks XREADGROUP + mpsc bridge. `ack_conn.publish` → `stream_publish(REDIS_STREAM_EXECUTOR_EVENTS)`. Dependência `futures_util::StreamExt` removida. 0 warnings.
  - **API WS bridge** (`redis_stream_subscriber`): `get_async_pubsub` + `subscribe` + `on_message` → XREADGROUP NOACK no grupo `ws-bridge`, dois streams (market_data + decisions) numa única chamada XREADGROUP.
  - **API kill-switch** handler: `conn.publish` → `stream_publish` para control_events; `subscribe` + `on_message` → `XREAD BLOCK 1000` (transient, sem grupo) filtrando por `request_id`.
  - `cargo check --workspace`: 0 warnings (10 pre-existing em strategy). `cargo test`: 172/172 + 1 ignored (E2E needs Redis).

### Concluído (Fase 6 — Reconciliação Auto-Fix)
- **Item 13 — Ativar auto-fix na reconciliação (live mode)**: Aprimorado `run_reconciliation_tick` com auto-fix via Bybit market orders quando `reconcile_auto_fix && trading_mode == Mainnet`. Limites configuráveis: `RECONCILE_MAX_CORRECTION_PCT` (5%) por tick (drift > limite é detect-only), `RECONCILE_MAX_DAILY` (5) por símbolo+side com reset diário via `reconcile_daily_counts`. Drift positivo (bybit > local) → submit market order ENTER_*. Drift negativo (local > bybit) → submit market order CLOSE_* + `apply_reconciliation_reduce_local`. 3 novas env vars: `EXECUTOR_RECONCILE_AUTO_FIX`, `RECONCILE_MAX_CORRECTION_PCT`, `RECONCILE_MAX_DAILY`. Logging obrigatório em todos os pontos de decisão. `cargo check --workspace` 0 warnings, 172/172 testes.

### Bloqueado
- Redis não instalado — E2E test requer Redis externo
- PostgreSQL roda local (senha: viper_secret_password)
