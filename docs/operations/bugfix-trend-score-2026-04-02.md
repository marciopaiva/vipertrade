# Bug Fix: min_trend_score_for_side - 2026-04-02

## 🐛 Bug Identificado

A função `min_trend_score_for_side` no `services/strategy/src/main.rs` estava verificando a configuração global **ANTES** da configuração por símbolo, ignorando completamente os filtros específicos de cada par.

### Código Anterior (Bug)

```rust
fn min_trend_score_for_side(&self, symbol: &str, side: &str) -> f64 {
    let side_key = if side.eq_ignore_ascii_case("short") {
        "min_trend_score_short"
    } else {
        "min_trend_score_long"
    };

    // ❌ BUG: Verifica global PRIMEIRO
    if let Some(value) = self.mode_f64(side_key) {
        return value;  // ← Retorna 0.40 (global PAPER) e ignora config por símbolo
    }

    // ✅ Config por símbolo (NUNCA alcançado se global existir)
    self.pair_cfg(symbol)
        .map(|v| cfg_f64(v, &["entry_filters", side_key], ...))
}
```

### Consequência

| Símbolo | Config Símbolo | Config Global | Usado (Bug) |
|---------|----------------|---------------|-------------|
| XRPUSDT | 0.50 | 0.40 | **0.40** ❌ |
| ADAUSDT | 0.50 | 0.40 | **0.40** ❌ |
| SUIUSDT | 0.55 | 0.40 | **0.40** ❌ |

**Resultado:** 17 LONGs entraram quando deveriam ser bloqueados, causando -0,27 USDT de perda.

---

## ✅ Correção Aplicada

### Código Novo (Correto)

```rust
fn min_trend_score_for_side(&self, symbol: &str, side: &str) -> f64 {
    let side_key = if side.eq_ignore_ascii_case("short") {
        "min_trend_score_short"
    } else {
        "min_trend_score_long"
    };

    // ✅ CORREÇÃO: Verifica configuração por símbolo PRIMEIRO
    if let Some(pair_value) = self.pair_cfg(symbol)
        .and_then(|v| cfg_get(v, &["entry_filters", side_key]))
        .and_then(Value::as_f64) {
        return pair_value;  // ← Retorna 0.50 (XRPUSDT), 0.55 (SUIUSDT), etc.
    }

    // ✅ Depois usa o global mode como fallback
    if let Some(value) = self.mode_f64(side_key) {
        return value;
    }

    // ✅ Fallback para configuração genérica do símbolo
    self.pair_cfg(symbol)
        .map(|v| cfg_f64(...))
        .unwrap_or_else(|| cfg_f64(&self.global, ...))
}
```

### Nova Ordem de Prioridade

1. **Primeiro:** `entry_filters.min_trend_score_long` do símbolo (ex: XRPUSDT → 0.50)
2. **Segundo:** `min_trend_score_long` do mode global (ex: PAPER → 0.50)
3. **Terceiro:** `entry_filters.min_trend_score_long` genérico do símbolo
4. **Fallback:** `entry_filters.min_trend_score_long` global (ex: 0.25)

---

## 📊 Impacto Esperado

### Antes do Fix

| Métrica | Valor |
|---------|-------|
| Filtro Long usado | 0.40 (global) |
| LONGs permitidos | 17 |
| LONGs win rate | 35% |
| LONGs PnL | -0,27 USDT |

### Depois do Fix

| Métrica | Valor Esperado |
|---------|----------------|
| Filtro Long XRPUSDT | 0.50 |
| Filtro Long ADAUSDT | 0.50 |
| Filtro Long SUIUSDT | 0.55 |
| LONGs permitidos | ~5-8 (redução 60%) |
| LONGs win rate | >50% |
| LONGs PnL | Positivo |

---

## 🔧 Arquivos Modificados

1. **`services/strategy/src/main.rs`**
   - Função `min_trend_score_for_side` corrigida (linhas 434-468)

2. **`config/trading/pairs.yaml`**
   - Global PAPER `min_trend_score_long`: 0.40 → **0.50**

---

## ✅ Validação

```bash
# Compilação
cargo check --package viper-strategy --locked
# ✅ Success

# Build Docker
docker compose -f compose/docker-compose.yml build strategy
# ✅ Image built

# Deploy
docker compose -f compose/docker-compose.yml up -d strategy
# ✅ Started
```

---

## 📋 Monitoramento Pós-Fix

### Sinais de Sucesso (24-48h)

- [ ] LONGs apenas em XRPUSDT, ADAUSDT, SUIUSDT, LTCUSDT (não em DOGE, LINK, NEAR)
- [ ] LONGs win rate > 50%
- [ ] LONGs PnL total positivo
- [ ] Redução de 60% no volume de LONGs

### Sinais de Alerta

- [ ] LONGs ainda entrando em DOGEUSDT/LINKUSDT/NEARUSDT → `allow_long` não funcionando
- [ ] Win rate LONG < 40% → Aumentar filtros para 0.60+
- [ ] Zero LONGs em 24h → Filtros muito rigorosos, reduzir para 0.45

---

**Data:** 2026-04-02  
**Autor:** ViperTrade Operations  
**Versão:** v2.2 (Bug Fix Crítico)  
**Status:** ✅ Deploy realizado
