# 06 - Validation and Checklists

Source: VIPERTRADE_SPEC.md (sections 18-20).

## Pre-Deploy Validation

- Seguranca: security-check passa, compose/.env com permissao restrita, .env fora do Git, API keys com privilegio minimo, 2FA e IP whitelist ativos.
- Database: schema aplicado, tabelas e indices existentes, rotina de backup definida.
- Servicos: containers sobem, health checks passam, Redis Pub/Sub operacional e reconnect de WebSocket validado.
- Risk: sizing e limites validados, stop loss e trailing stop funcionando, circuit breaker e daily loss testados.
- Notificacoes: webhook configurado, alertas criticos e warning entregues.
- Testes: paper trading estavel, backtest de estresse aprovado, kill switch e error handling testados.
- Smart Copy e Lead Trader: sizing no range alvo, leverage por perfil, conta e metricas de leader prontas.

## Runbook Commands

- Inicializacao e seguranca: ./scripts/init-secrets.sh, ./scripts/security-check.sh
- Compose (Docker): ./scripts/compose.sh up -d --build, ./scripts/compose.sh ps, ./scripts/compose.sh logs -f, ./scripts/compose.sh down
- Compose (fallback legado Podman): cd compose && podman-compose up --build -d, podman-compose ps, podman-compose logs -f, podman-compose down
- Backtest: ./scripts/run-backtest.sh MEDIUM 2025-02-01 2026-02-28
- API e operacao: status, posicoes, trades, stats do leader e kill-switch via endpoints HTTP
- Database: acesso SQL no container postgres para consultas operacionais

## API Surface (Current Spec)

- Portfolio, positions, trades e performance.
- System status e kill-switch.
- Endpoints de copy trading e leader profile.
- Eventos WebSocket para portfolio, posicoes, trades e alertas.

## Referencia Original

- VIPERTRADE_SPEC.md linhas aproximadas 1768-1980.
