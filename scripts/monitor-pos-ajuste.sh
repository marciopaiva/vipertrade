#!/bin/bash
# ViperTrade - Monitoramento Pós-Ajuste
# Uso: ./monitor-pos-ajuste.sh [intervalo_segundos]

INTERVAL=${1:-60}
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}       ViperTrade - Monitoramento Pós-Ajuste${NC}"
echo -e "${BLUE}       Config v2.0 - 2026-04-01${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"
echo ""

while true; do
    clear
    echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  ViperTrade - Monitoramento em Tempo Real${NC}"
    echo -e "${BLUE}  Atualizado: $(date '+%Y-%m-%d %H:%M:%S')${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"
    echo ""

    # Header info
    echo -e "${YELLOW}📊 VISÃO GERAL${NC}"
    echo "─────────────────────────────────────────────────────────────"
    
    docker exec vipertrade-postgres psql -U viper -d vipertrade -t -c "
        SELECT 
            'Total Trades: ' || COUNT(*) || 
            ' | Abertos: ' || COUNT(*) FILTER (WHERE status='open') ||
            ' | Win Rate: ' || ROUND(COUNT(*) FILTER (WHERE pnl > 0)::numeric / NULLIF(COUNT(*) FILTER (WHERE status='closed'), 0) * 100, 1) || '%' ||
            ' | PnL Total: ' || ROUND(COALESCE(SUM(pnl) FILTER (WHERE status='closed'), 0)::numeric, 4) || ' USDT'
        FROM trades 
        WHERE opened_at > '2026-04-02 12:00:00';
    " 2>/dev/null | xargs
    
    echo ""
    echo -e "${YELLOW}📈 PERFORMANCE POR MOTIVO DE FECHAMENTO${NC}"
    echo "─────────────────────────────────────────────────────────────"
    
    docker exec vipertrade-postgres psql -U viper -d vipertrade -t -c "
        SELECT 
            RPAD(close_reason, 20, ' ') || 
            ' | Qtd: ' || LPAD(COUNT(*)::text, 3, ' ') ||
            ' | %: ' || LPAD(ROUND(COUNT(*)::numeric / SUM(COUNT(*)) OVER () * 100, 1)::text, 5, ' ') ||
            ' | Avg PnL: ' || LPAD(ROUND(AVG(pnl_pct)::numeric, 3)::text, 7, ' ') ||
            ' | Total: ' || ROUND(SUM(pnl)::numeric, 4)
        FROM trades 
        WHERE status='closed' 
          AND opened_at > '2026-04-02 12:00:00'
        GROUP BY close_reason
        ORDER BY COUNT(*) DESC;
    " 2>/dev/null
    
    echo ""
    echo -e "${YELLOW}💹 PERFORMANCE POR SÍMBOLO${NC}"
    echo "─────────────────────────────────────────────────────────────"
    
    docker exec vipertrade-postgres psql -U viper -d vipertrade -t -c "
        SELECT 
            RPAD(symbol, 10, ' ') || 
            ' | Trades: ' || LPAD(COUNT(*)::text, 3, ' ') ||
            ' | Wins: ' || LPAD(COUNT(*) FILTER (WHERE pnl > 0)::text, 3, ' ') ||
            ' | Avg: ' || LPAD(ROUND(AVG(pnl_pct)::numeric, 3)::text, 7, ' ') ||
            ' | Total: ' || LPAD(ROUND(SUM(pnl)::numeric, 4)::text, 8, ' ')
        FROM trades 
        WHERE status='closed' 
          AND opened_at > '2026-04-02 12:00:00'
        GROUP BY symbol
        ORDER BY SUM(pnl) DESC NULLS LAST;
    " 2>/dev/null
    
    echo ""
    echo -e "${YELLOW}⏱️ DURAÇÃO MÉDIA DAS OPERAÇÕES${NC}"
    echo "─────────────────────────────────────────────────────────────"
    
    docker exec vipertrade-postgres psql -U viper -d vipertrade -t -c "
        SELECT 
            RPAD(close_reason, 20, ' ') ||
            ' | Avg: ' || LPAD(ROUND(AVG(duration_seconds)/60, 1)::text, 5, ' ') || ' min' ||
            ' | Min: ' || LPAD(ROUND(MIN(duration_seconds)/60, 1)::text, 5, ' ') ||
            ' | Max: ' || LPAD(ROUND(MAX(duration_seconds)/60, 1)::text, 5, ' ')
        FROM trades 
        WHERE status='closed' 
          AND opened_at > '2026-04-02 12:00:00'
        GROUP BY close_reason;
    " 2>/dev/null
    
    echo ""
    echo -e "${YELLOW}📊 LONG VS SHORT${NC}"
    echo "─────────────────────────────────────────────────────────────"
    
    docker exec vipertrade-postgres psql -U viper -d vipertrade -t -c "
        SELECT 
            RPAD(side, 6, ' ') ||
            ' | Trades: ' || LPAD(COUNT(*)::text, 3, ' ') ||
            ' | Win Rate: ' || LPAD(ROUND(COUNT(*) FILTER (WHERE pnl > 0)::numeric / NULLIF(COUNT(*), 0) * 100, 1)::text, 5, ' ') || '%' ||
            ' | Avg: ' || LPAD(ROUND(AVG(pnl_pct)::numeric, 3)::text, 7, ' ') ||
            ' | Total: ' || ROUND(SUM(pnl)::numeric, 4)
        FROM trades 
        WHERE status='closed' 
          AND opened_at > '2026-04-02 12:00:00'
        GROUP BY side;
    " 2>/dev/null
    
    echo ""
    echo -e "${YELLOW}🕐 TRADES POR HORA (últimas 12h)${NC}"
    echo "─────────────────────────────────────────────────────────────"
    
    docker exec vipertrade-postgres psql -U viper -d vipertrade -t -c "
        SELECT 
            LPAD(EXTRACT(HOUR FROM opened_at)::text || ':00', 6, ' ') ||
            ' | Qtd: ' || LPAD(COUNT(*)::text, 3, ' ') ||
            ' | Thesis: ' || LPAD(COUNT(*) FILTER (WHERE close_reason='thesis_invalidated')::text, 3, ' ') ||
            ' | Trail: ' || LPAD(COUNT(*) FILTER (WHERE close_reason='trailing_stop')::text, 3, ' ') ||
            ' | PnL: ' || LPAD(ROUND(SUM(pnl)::numeric, 3)::text, 8, ' ')
        FROM trades 
        WHERE opened_at > NOW() - INTERVAL '12 hours'
          AND opened_at > '2026-04-02 12:00:00'
        GROUP BY EXTRACT(HOUR FROM opened_at)
        ORDER BY EXTRACT(HOUR FROM opened_at);
    " 2>/dev/null
    
    echo ""
    echo -e "${YELLOW}🔍 ÚLTIMOS 5 TRADES${NC}"
    echo "─────────────────────────────────────────────────────────────"
    
    docker exec vipertrade-postgres psql -U viper -d vipertrade -t -c "
        SELECT 
            RPAD(symbol, 10, ' ') ||
            RPAD(side, 6, ' ') ||
            RPAD(close_reason, 20, ' ') ||
            ' | PnL: ' || LPAD(ROUND(pnl_pct, 2)::text, 7, ' ') ||
            ' | Duração: ' || LPAD(ROUND(duration_seconds/60, 0)::text, 4, ' ') || 'm' ||
            ' | ' || TO_CHAR(opened_at, 'HH24:MI')
        FROM trades 
        WHERE opened_at > '2026-04-02 12:00:00'
        ORDER BY opened_at DESC 
        LIMIT 5;
    " 2>/dev/null
    
    echo ""
    echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"
    echo -e "${GREEN}Refresh: ${INTERVAL}s | Press Ctrl+C para sair${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}"
    
    sleep $INTERVAL
done
