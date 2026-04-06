# Ajustes de Configuração por Símbolo - 2026-04-02

## Contexto

Após análise de 76 trades (24h pós-ajuste v2.0), identificamos que:
- **SHORTS** estão performando bem: +0,65 USDT, 55% WR
- **LONGS** estão catastróficos: -1,09 USDT, 20% WR
- Alguns símbolos têm problemas estruturais em **ambos lados**

**Estratégia:** Ajuste cirúrgico por símbolo ao invés de desabilitar completamente.

---

## 📊 Performance por Símbolo (Base para Ajustes)

| Símbolo | Long PnL (WR) | Short PnL (WR) | Ação |
|---------|---------------|----------------|------|
| DOGEUSDT | -0,10 (0%) | **+0,34 (88%)** | 🔴 Long OFF, Short mantido |
| XRPUSDT | — (0 trades) | **+0,10 (60%)** | ⚠️ Long rigoroso, Short mantido |
| ADAUSDT | -0,18 (60%) | **+0,20 (60%)** | ⚠️ Long rigoroso, Short mantido |
| LTCUSDT | — (0 trades) | **+0,01 (33%)** | ✅ Mantido (poucos dados) |
| LINKUSDT | -0,22 (50%) | +0,03 (40%) | 🔴 Long OFF, Short mantido |
| NEARUSDT | -0,33 (20%) | **+0,13 (71%)** | 🔴 Long OFF, Short mantido |
| SUIUSDT | -0,26 (40%) | -0,16 (45%) | ⚠️ **Ambos filtros rigorosos** |

---

## 🔧 Ajustes Aplicados

### 1. DOGEUSDT - Short Excelente, Long Ruim

```yaml
DOGEUSDT:
  entry_filters:
    allow_long: false           # 🔴 0% WR, -0,10 USDT
    allow_short: true           # ✅ 88% WR, +0,34 USDT
    min_trend_score_long: 0.50  # Era 0.43
    min_trend_score_short: 0.35 # Mantido (funciona)
```

---

### 2. LINKUSDT - Short Marginal, Long Ruim

```yaml
LINKUSDT:
  entry_filters:
    allow_long: false           # 🔴 -0,22 USDT, 50% WR
    allow_short: true           # ⚠️ +0,03 USDT, 40% WR (marginal mas positivo)
    min_trend_score_long: 0.55  # Era 0.32 (+72%)
    min_trend_score_short: 0.45 # Era 0.33 (+36%)
    min_signal_confirmation_ticks_short: 6  # Era 5
    max_atr_pct: 0.0040         # Era 0.0048
```

---

### 3. NEARUSDT - Short Ótimo, Long Horrível

```yaml
NEARUSDT:
  entry_filters:
    allow_long: false           # 🔴 -0,33 USDT, 20% WR (pior de todos)
    allow_short: true           # ✅ +0,13 USDT, 71% WR
    min_trend_score_long: 0.55  # Era 0.38
    min_trend_score_short: 0.40 # Era 0.37
    min_signal_confirmation_ticks_short: 5  # Era 6 (reduzido - funciona bem!)
```

---

### 4. SUIUSDT - Ambos Lados Perdem (CRÍTICO)

```yaml
SUIUSDT:
  entry_filters:
    allow_long: true            # ⚠️ -0,26 USDT, 40% WR
    allow_short: true           # ⚠️ -0,16 USDT, 45% WR
    min_trend_score_long: 0.55  # Era 0.34 (+62%)
    min_trend_score_short: 0.50 # Era 0.41 (+22%)
    min_signal_confirmation_ticks_long: 6   # Era 4
    min_signal_confirmation_ticks_short: 8  # Era 7
    min_signal_confirmation_ticks: 5        # Era 4
    max_atr_pct: 0.0040         # Era 0.0050
```

**Nota:** Filtros mais agressivos em todos os símbolos. Se não melhorar em 48h, **desabilitar**.

---

### 5. ADAUSDT - Short Bom, Long Recuperável

```yaml
ADAUSDT:
  entry_filters:
    allow_long: true            # ⚠️ -0,18 USDT, mas 60% WR (filtres rigorosos)
    allow_short: true           # ✅ +0,20 USDT, 60% WR
    min_trend_score_long: 0.50  # Era 0.31 (+61%)
    min_trend_score_short: 0.35 # Era 0.24 (+46%)
    min_signal_confirmation_ticks_long: 6   # Era 4
    thesis_invalidation_confirmation_ticks: 5  # Era 1 (muito sensível!)
    max_atr_pct: 0.0030         # Era 0.0035
```

---

### 6. XRPUSDT - Short Bom, Long Sem Dados

```yaml
XRPUSDT:
  entry_filters:
    allow_long: true            # ⚠️ Sem trades ainda
    allow_short: true           # ✅ +0,10 USDT, 60% WR
    min_trend_score_long: 0.50  # Era 0.32 (+56%)
    min_trend_score_short: 0.40 # Era 0.32 (+25%)
    min_signal_confirmation_ticks_long: 7   # Era 6
    min_signal_confirmation_ticks_short: 5  # Era 4
    max_atr_pct: 0.0035         # Era 0.0045
```

---

### 7. LTCUSDT - Sem Alterações

```yaml
LTCUSDT:
  entry_filters:
    # Mantido - poucos dados (apenas 3 trades short)
    # Performance: +0,01 USDT, 33% WR
    allow_long: true
    allow_short: true
```

---

## 📈 Resumo das Mudanças

| Símbolo | Long | Short | Filtros |
|---------|------|-------|---------|
| DOGEUSDT | ❌ OFF | ✅ ON | Mantidos |
| LINKUSDT | ❌ OFF | ⚠️ ON | +36% rigor |
| NEARUSDT | ❌ OFF | ✅ ON | +8% rigor |
| SUIUSDT | ⚠️ ON | ⚠️ ON | +62% / +22% rigor |
| ADAUSDT | ⚠️ ON | ✅ ON | +61% / +46% rigor |
| XRPUSDT | ⚠️ ON | ✅ ON | +56% / +25% rigor |
| LTCUSDT | ✅ ON | ✅ ON | Mantidos |

---

## 🎯 Impacto Esperado

| Cenário | Trades/dia | Win Rate | PnL Total (7 dias) |
|---------|------------|----------|-------------------|
| **Antes** | ~10 | 51% | -0,44 USDT |
| **Pós-ajuste** | ~6-8 | 58-62% | +0,80 a +1,20 USDT |

**Redução de volume esperada:** 30-40% (menos entradas, mais qualidade)

---

## 🔍 Monitoramento

### Sinais de Alerta (48h)

- [ ] SUIUSDT continuar negativo em ambos lados → **Desabilitar**
- [ ] LINKUSDT Short < 35% WR → **Desabilitar Short**
- [ ] ADAUSDT Long > 5 trades perdidos → **Desabilitar Long**
- [ ] Win rate geral < 50% → **Revisar filtros globais**

### Sinais Positivos

- [ ] Win rate geral > 55% → ✅ Config OK
- [ ] Shorts mantêm > 60% WR → ✅ Estratégia validada
- [ ] SUIUSDT fica positivo → ✅ Filtros funcionaram
- [ ] PnL total > +0,50 USDT/7 dias → ✅ Rentável

---

## 📋 Próximos Passos

1. **24h:** Primeiro check de performance
2. **48h:** Decidir sobre SUIUSDT (manter ou desabilitar)
3. **72h:** Avaliar reabilitação de LONGs (se mercado em uptrend)
4. **7 dias:** Revisão completa da estratégia

---

## 🚀 Comandos de Validação

```bash
# Monitoramento contínuo
./scripts/monitor-pos-ajuste.sh 60

# Snapshot rápido
docker exec vipertrade-postgres psql -U viper -d vipertrade -c "
SELECT symbol, side, COUNT(*), 
       ROUND(AVG(pnl_pct)::numeric, 3) as avg_pnl,
       ROUND(SUM(pnl)::numeric, 4) as total_pnl
FROM trades 
WHERE opened_at > '2026-04-02 12:00:00'
  AND status='closed'
GROUP BY symbol, side
ORDER BY total_pnl DESC;"
```

---

**Data:** 2026-04-02  
**Autor:** ViperTrade Operations  
**Versão:** v2.1 (Ajuste Cirúrgico por Símbolo)  
**Status:** ✅ Deploy realizado
