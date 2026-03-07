# 05 - Runtime and Operations

Source: VIPERTRADE_SPEC.md (sections 7-17).

## Error Handling and Resilience

- Matriz de erro por dominio: Bybit REST, WebSocket, database e risk engine.
- Politica de retry com backoff exponencial e jitter.
- Falhas criticas devem acionar pausa de entradas e alerta imediato.
- Fallback operacional: polling REST quando WebSocket ficar indisponivel.

## WebSocket Reconnection Strategy

- Reconexao progressiva para canais publicos e privados.
- Heartbeat com timeout e resubscription automatica.
- Recuperacao de estado apos reconnect.
- Validar posicoes, ordens e reconciliar via REST.

## Disaster Recovery

- Classificacao de incidentes: critical, high, medium, low.
- SLOs operacionais definidos por RTO/RPO.
- Procedimentos obrigatorios.
- kill_switch para conter perdas.
- Restauracao de banco com reconciliacao posterior.
- Revogacao de API key em suspeita de comprometimento.
- Post-mortem obrigatorio para incidentes critical e high.

## Secrets and Security Operations

- Segredos em compose/.env e pasta secrets/ com permissoes restritas.
- Rotacao de keys em ciclo regular (exemplo: 90 dias) com teste em testnet.
- Checklist de pre-mainnet inclui permissao minima de API key, 2FA, IP whitelist e ausencia de segredos no Git.

## Notifications and Monitoring

- Alertas por webhook com niveis critical, warning e info.
- Dedupe e batching para reduzir ruido operacional.
- Tipos de alerta principais: circuit breaker, stop loss, trailing stop e resumo diario.
- Alertas operacionais sao para operador do bot; eventos de copy para followers sao controlados pela Bybit.

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

- VIPERTRADE_SPEC.md linhas aproximadas 491-1767.
