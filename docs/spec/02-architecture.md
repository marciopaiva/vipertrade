# 02 - Architecture

Source: `VIPERTRADE_SPEC.md` (section 2).

## Topologia

- `market-data`: ingestao de WS da Bybit e normalizacao.
- `strategy`: avaliacao de estrategia com Tupa.
- `executor`: execucao de ordens.
- `monitor`: health, alertas e reconciliacao.
- `postgres`: estado persistente.
- `redis`: pub/sub e cache.

## Servicos e Responsabilidades

- `postgres`: trades, snapshots de posicao, eventos e metricas.
- `redis`: transporte de eventos e estado transiente.
- `api` e `web`: plano de leitura/controle.

## Fluxo de Decisao

1. Market data da Bybit entra no `market-data`.
2. Evento normalizado vai para `redis`.
3. `strategy` consome, avalia e produz decisao.
4. `executor` valida/executa ordem na Bybit.
5. Resultado persiste em banco e alimenta monitoramento.
6. `monitor` executa reconciliacao periodica.

## Deploy Local (WSL + Podman)

- Compose bridge e padrao.
- Compose host e fallback.
- Health checks por servico.

## Referencia Original

- `VIPERTRADE_SPEC.md` linhas aproximadas 87-161.