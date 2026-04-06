# 📊 Dashboard de Monitoramento - Pós-Ajuste v2.0

## 🎯 Status Atual

| Item | Status |
|------|--------|
| **Configuração** | v2.0 (2026-04-01) |
| **Modo** | PAPER (mainnet data) |
| **Perfil** | MEDIUM |
| **Banco de Dados** | ✅ Limpo |
| **Serviços** | ✅ Running |

---

## 📋 Ajustes Aplicados

### Críticos
- ✅ `thesis_invalidation_confirmation_ticks`: 2 → **4**
- ✅ `thesis_invalidation_cooldown_long`: 3 → **8 min**
- ✅ `thesis_invalidation_cooldown_short`: 3 → **10 min**

### Filtros de Entrada
- ✅ `min_trend_score_long`: 0.28 → **0.40**
- ✅ `min_trend_score_short`: 0.34 → **0.45**
- ✅ `btc_macro_min_trend_score_long`: 0.05 → **0.15**
- ✅ `btc_macro_min_trend_score_short`: 0.10 → **0.20**

### Confirmação
- ✅ `min_signal_confirmation_ticks_long`: 3 → **4**
- ✅ `min_signal_confirmation_ticks_short`: 6 → **5**
- ✅ `min_hold_seconds`: 180 → **300**

---

## 🎯 Metas

| Métrica | Cenário Anterior | Meta |
|---------|------------------|------|
| Thesis Invalidations | 61% | < 40% |
| Trailing Stop Exits | 39% | > 50% |
| Win Rate | ~48% | > 55% |
| PnL Médio/Trade | -0,0164% | > 0,10% |

---

## 🔧 Comandos de Monitoramento

### Monitoramento Contínuo (30s refresh)
```bash
cd /home/paiva/teste/vipertrade
./scripts/monitor-pos-ajuste.sh 30
```

### Snapshot Rápido
```bash
docker exec vipertrade-postgres psql -U viper -d vipertrade -c "
SELECT 
    close_reason,
    COUNT(*) as qtd,
    ROUND(COUNT(*)::numeric / SUM(COUNT(*)) OVER () * 100, 1) as pct,
    ROUND(AVG(pnl_pct)::numeric, 3) as avg_pnl_pct,
    ROUND(SUM(pnl)::numeric, 4) as total_pnl
FROM trades 
WHERE status='closed' 
  AND opened_at > '2026-04-01 12:30:00'
GROUP BY close_reason
ORDER BY COUNT(*) DESC;
"
```

### Performance por Símbolo
```bash
docker exec vipertrade-postgres psql -U viper -d vipertrade -c "
SELECT 
    symbol,
    COUNT(*) as trades,
    COUNT(*) FILTER (WHERE pnl > 0) as wins,
    ROUND(AVG(pnl_pct)::numeric, 3) as avg_pnl,
    ROUND(SUM(pnl)::numeric, 4) as total_pnl
FROM trades 
WHERE status='closed' 
  AND opened_at > '2026-04-01 12:30:00'
GROUP BY symbol
ORDER BY total_pnl DESC;
"
```

### Trades por Hora
```bash
docker exec vipertrade-postgres psql -U viper -d vipertrade -c "
SELECT 
    EXTRACT(HOUR FROM opened_at) as hora,
    COUNT(*) as trades,
    COUNT(*) FILTER (WHERE close_reason='thesis_invalidated') as thesis,
    COUNT(*) FILTER (WHERE close_reason='trailing_stop') as trailing,
    ROUND(SUM(pnl)::numeric, 3) as pnl_total
FROM trades 
WHERE opened_at > '2026-04-01 12:30:00'
GROUP BY EXTRACT(HOUR FROM opened_at)
ORDER BY hora;
"
```

### Long vs Short
```bash
docker exec vipertrade-postgres psql -U viper -d vipertrade -c "
SELECT 
    side,
    COUNT(*) as trades,
    ROUND(COUNT(*) FILTER (WHERE pnl > 0)::numeric / NULLIF(COUNT(*), 0) * 100, 1) as win_rate,
    ROUND(AVG(pnl_pct)::numeric, 3) as avg_pnl,
    ROUND(SUM(pnl)::numeric, 4) as total_pnl
FROM trades 
WHERE status='closed' 
  AND opened_at > '2026-04-01 12:30:00'
GROUP BY side;
"
```

---

## 📝 Log de Observações

### Dia 1 (2026-04-01)
- [ ] Aguardando primeiros trades...

### Dia 2 (2026-04-02)
- [ ] Preencher após 24h...

### Dia 3 (2026-04-03)
- [ ] Preencher após 48h...

---

## 🚨 Alertas

Verificar se:
- [ ] Thesis invalidations > 50% após 24h → Aumentar para 5-6 ticks
- [ ] Win rate < 45% após 48h → Revisar filtros de entrada
- [ ] 5+ trades consecutivas perdedoras → Verificar circuit breaker
- [ ] PnL total < -0.50 USDT → Considerar rollback

---

## 📞 Contatos

- **Documentação:** `docs/operations/ajustes-config-2026-04-01.md`
- **Config:** `config/trading/pairs.yaml`
- **Script Monitor:** `scripts/monitor-pos-ajuste.sh`
