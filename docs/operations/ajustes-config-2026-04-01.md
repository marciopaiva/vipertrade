# Ajustes de Configuração - 2026-04-01

## Contexto

Após análise de 198 trades no modo PAPER (dados mainnet), identificamos que **61,1% das operações** estavam fechando por **thesis_invalidated** com perda média de **-0,32%**, enquanto as operações fechadas por **trailing_stop** tinham lucro médio de **+0,47%**.

**Diagnóstico:** Filtros de entrada muito permissivos + thesis invalidation excessivamente sensível = saídas prematuras.

---

## Resumo das Mudanças

### 1. Thesis Invalidation (Crítico)

| Parâmetro | Antes | Depois | Impacto |
|-----------|-------|--------|---------|
| `thesis_invalidation_confirmation_ticks` | 2 | **4** | Requer 4 velas contra (era 2) |
| `thesis_invalidation_cooldown_minutes_long` | 3 | **8** | Cooldown 167% maior |
| `thesis_invalidation_cooldown_minutes_short` | 3 | **10** | Cooldown 233% maior |

**Arquivos:** `config/trading/pairs.yaml` (PAPER, MAINNET, DOGEUSDT, ADAUSDT)

---

### 2. Filtros de Entrada (Alto)

| Parâmetro | Antes | Depois | Impacto |
|-----------|-------|--------|---------|
| `min_trend_score_long` | 0.28 | **0.40** | +43% mais rigoroso |
| `min_trend_score_short` | 0.34 | **0.45** | +32% mais rigoroso |
| `min_trend_score` (global) | 0.25 | **0.35** | +40% mais rigoroso |

**Arquivos:** `config/trading/pairs.yaml` (global PAPER, global MAINNET)

---

### 3. Confirmação de Sinal (Médio)

| Parâmetro | Antes | Depois | Impacto |
|-----------|-------|--------|---------|
| `min_signal_confirmation_ticks_long` | 3 | **4** | +1 tick confirmação |
| `min_signal_confirmation_ticks_short` | 6 | **5** | -1 tick (reduz assimetria) |
| `min_signal_confirmation_ticks` | 2 | **4** | +2 ticks global |

**Arquivos:** `config/trading/pairs.yaml` (global PAPER, global MAINNET, global entry_filters)

---

### 4. BTC Macro Alignment (Médio)

| Parâmetro | Antes | Depois | Impacto |
|-----------|-------|--------|---------|
| `btc_macro_min_trend_score_long` | 0.05 | **0.15** | 3x mais rigoroso |
| `btc_macro_min_trend_score_short` | 0.10 | **0.20** | 2x mais rigoroso |

**Arquivos:** `config/trading/pairs.yaml` (global PAPER, global MAINNET, global entry_filters)

---

### 5. Hold Mínimo (Baixo)

| Parâmetro | Antes | Depois | Impacto |
|-----------|-------|--------|---------|
| `min_hold_seconds` | 180 | **300** | +67% tempo mínimo |

**Arquivos:** `config/trading/pairs.yaml` (global PAPER, global MAINNET)

---

### 6. Cooldown de Stop Loss (Baixo)

| Parâmetro | Antes | Depois | Impacto |
|-----------|-------|--------|---------|
| `stop_loss_cooldown_minutes` | 3 | **5** | +67% cooldown |
| `stop_loss_cooldown_minutes_short` | 5 | **8** | +60% cooldown |

**Arquivos:** `config/trading/pairs.yaml` (global entry_filters)

---

## Arquivos Modificados

```
config/trading/pairs.yaml
├── global.mode_profiles.PAPER (otimizado)
├── global.mode_profiles.MAINNET (otimizado)
├── global.entry_filters (otimizado)
├── DOGEUSDT.entry_filters (thesis_invalidation: 1→4)
└── ADAUSDT.entry_filters (thesis_invalidation: 1→4)
```

---

## Impacto Esperado

| Métrica | Cenário Anterior | Cenário Esperado |
|---------|------------------|-----------------|
| Thesis Invalidations | 61% | ~35% |
| Trailing Stop Exits | 39% | ~55% |
| Win Rate | ~48% | ~58% |
| PnL Médio/Trade | -0,0164% | +0,15% |
| PnL Total (198 trades) | -0,09 USDT | +0,80 USDT |

---

## Rollback (se necessário)

Para reverter as mudanças:

```bash
cd /home/paiva/teste/vipertrade
git checkout config/trading/pairs.yaml
docker restart vipertrade-strategy vipertrade-executor
```

---

## Validação Pós-Deploy

Após 24-48 horas de operação, verificar:

```sql
-- Taxa de thesis invalidations
SELECT 
  close_reason, 
  COUNT(*) as count, 
  ROUND(COUNT(*)::numeric / SUM(COUNT(*)) OVER () * 100, 2) as pct
FROM trades 
WHERE status='closed' 
  AND opened_at > '2026-04-01 12:00:00'
GROUP BY close_reason;

-- Duração média das operações
SELECT 
  close_reason,
  ROUND(AVG(duration_seconds), 0) as avg_duration_sec,
  ROUND(AVG(pnl_pct)::numeric, 4) as avg_pnl_pct
FROM trades 
WHERE status='closed'
  AND opened_at > '2026-04-01 12:00:00'
GROUP BY close_reason;
```

---

## Próximos Passos

1. **Monitorar 24-48h** - Validar redução em thesis invalidations
2. **Ajuste fino** - Se thesis_invalidations > 45%, aumentar para 5-6 ticks
3. **Análise por símbolo** - Identificar pares com performance anômala
4. **Filtro temporal** - Considerar blackout em horários voláteis (00:00, 09:00, 15:00, 21:00 UTC)

---

## Assinatura

**Data:** 2026-04-01  
**Autor:** ViperTrade Operations  
**Versão:** v2.0  
**Status:** ✅ Deploy realizado
