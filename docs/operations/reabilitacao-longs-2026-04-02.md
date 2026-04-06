# Reabilitação de LONGs - 2026-04-02

## Contexto

Após correção do bug `min_trend_score_for_side` (v2.2), os filtros por símbolo
agora funcionam corretamente. Podemos reabilitar LONGs com filtros rigorosos.

## Símbolos Reabilitados

| Símbolo | allow_long | min_trend_score_long | Confirmação |
|---------|------------|---------------------|-------------|
| DOGEUSDT | ✅ true | **0.55** | 6 ticks |
| LINKUSDT | ✅ true | **0.50** | 6 ticks |
| NEARUSDT | ✅ true | **0.55** | 8 ticks |

## Filtros Aplicados

### DOGEUSDT
- `min_trend_score_long: 0.55` (filtro mais forte)
- `min_signal_confirmation_ticks_long: 6`
- Histórico anterior: 0% WR com filtro 0.43 (bug)

### LINKUSDT
- `min_trend_score_long: 0.50`
- `min_signal_confirmation_ticks_long: 6`
- Histórico anterior: 50% WR com filtro 0.32 (bug)

### NEARUSDT
- `min_trend_score_long: 0.55` (filtro mais forte)
- `min_signal_confirmation_ticks_long: 8` (máximo rigor)
- Histórico anterior: 20% WR com filtro 0.38 (bug)

## Monitoramento

Verificar após 24h:
- [ ] LONGs entrando apenas com trend_score >= filtro configurado
- [ ] Win rate LONG > 45%
- [ ] PnL LONG positivo por símbolo

## Rollback

Se LONGs continuarem perdendo após o fix:
```yaml
# Por símbolo
allow_long: false
```
