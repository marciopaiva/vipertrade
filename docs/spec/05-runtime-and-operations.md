# 05 - Runtime and Operations

Source: `docs/legacy/VIPERTRADE_SPEC.md` (sections 7-17).

## Error Handling and Resilience

- Error matrix by domain: Bybit REST, WebSocket, database, and risk engine.
- Retry policy with exponential backoff and jitter.
- Critical failures must pause new entries and trigger immediate alerting.
- Operational fallback: REST polling when WebSocket becomes unavailable.

## WebSocket Reconnection Strategy

- Progressive reconnection for public and private channels.
- Heartbeat with timeout and automatic resubscription.
- State recovery after reconnect.
- Validate positions and orders, then reconcile via REST.

## Disaster Recovery

- Incident classification: critical, high, medium, low.
- Operational SLOs defined by RTO/RPO.
- Mandatory procedures.
- `kill_switch` to contain losses.
- Database restore followed by reconciliation.
- API key revocation when compromise is suspected.
- Mandatory post-mortem for critical and high incidents.

## Secrets and Security Operations

- Secrets stored in `compose/.env` and `secrets/` with restricted permissions.
- Key rotation on a regular cadence (for example, every 90 days) with testnet validation.
- Pre-mainnet checklist includes minimum API key permissions, 2FA, IP allowlists, and no secrets committed to Git.

## Notifications and Monitoring

- Webhook alerts with `critical`, `warning`, and `info` levels.
- Deduplication and batching to reduce operational noise.
- Main alert types: circuit breaker, stop loss, trailing stop, and daily summary.
- Operational alerts target the bot operator; copy-trading events for followers remain controlled by Bybit.

## Tupa Integration Model

- Integracao de estrategia via pipeline .tp versionado.
- Execucao via binario standalone tupa, com I/O JSON.
- Regras de risco e validacao declarativas na pipeline.
- Saida com hash de execucao para auditabilidade.

## Trading Operations and Validation Modes

- Operacao como Lead Trader no Copy Trading Classic.
- Otimizacao Smart Copy com sizing previsivel, controle de slippage e limites de alavancagem por perfil.
- Protecao contra auto-unfollow com reducao de failed copies e menor variacao abrupta de sizing.
- Modos de validacao antes de producao: backtest de estresse e paper trading com dados reais e execucao simulada.

## Dynamic Trailing Stop

- Ativacao por lucro minimo e ajuste progressivo (ratcheting).
- Trail nunca afrouxa; apenas mantem ou aperta.
- Parametros por perfil de risco para equilibrar protecao e captura de tendencia.
- Integracao com amend_order e fallback de retry.

## Development Blocks

- Blocos 1-15 estruturam entrega incremental.
- Base de projeto e compose.
- Servicos core (market-data, strategy, executor, monitor).
- Tratamento de erro e testes.
- Documentacao, deploy micro e otimizacoes Smart Copy e trailing.

## Referencia Original

- `docs/legacy/VIPERTRADE_SPEC.md`, approximate lines 491-1767.
