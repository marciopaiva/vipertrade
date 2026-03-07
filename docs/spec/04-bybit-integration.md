# 04 - Bybit Integration

Source: `VIPERTRADE_SPEC.md` (sections 6 and 8).

## Canais de Integracao

- REST para execucao de ordens e estado.
- WebSocket para market data e streams operacionais.

## Requisitos

- Idempotencia em envio de ordens (`order_link_id`).
- Retry com backoff para falhas transientes.
- Reconexao de WebSocket com estrategia progressiva.

## Observabilidade

- Registrar latencia de chamadas REST.
- Registrar eventos de reconexao e downtime de WS.
- Correlacionar falhas com simbolo e tipo de ordem.

## Seguranca

- Credenciais via `.env` / secrets only.
- Nunca persistir segredo em logs.

## Referencia Original

- `VIPERTRADE_SPEC.md` secoes 6 e 8.