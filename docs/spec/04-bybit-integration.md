# 04 - Bybit Integration

## Integration Channels

ViperTrade integrates with Bybit (and, for cross-exchange consensus, Binance and OKX)
over **REST only** — there is no exchange WebSocket consumer:

- **Public market data**: REST kline polling. `market-data` fetches `/v5/market/kline`
  (and the Binance/OKX equivalents) on a fixed cadence (~5s cycle) and normalizes the
  candles into a `MarketSignal`. Cross-exchange consensus requires the symbol to be live
  on bybit ∩ binance ∩ okx.
- **Private trading**: REST for order submission, position/account queries, and native
  trailing. Signed via the Bybit v5 private endpoints.

> The only WebSocket in the system is the **`api` service's own WS server**
> (`ws://…/ws`), which fans Redis events out to the web dashboard. It is not a Bybit
> stream.

## Base URLs

- Mainnet REST: `https://api.bybit.com`
- Testnet REST: `https://api-testnet.bybit.com`

Paper mode reads **mainnet public prices** (real prices) but never submits orders; the
wallet and fills are simulated in the database.

## Requirements

- Idempotent order submission via `order_link_id` (UUID-based).
- Retry with bounded backoff for transient failures.
- `BYBIT_RECV_WINDOW` (default 5000 ms) bounds request signature validity.
- Unified account type (`BYBIT_ACCOUNT_TYPE=UNIFIED`).

## Observability

- Log REST call latency and error codes per endpoint.
- Correlate failures by symbol, order type, and error code.

## Security

- API keys via `.env` (local) or Kubernetes secrets (Kind); never committed to Git.
- Mode-scoped credentials (`BYBIT_TESTNET_*` / `BYBIT_MAINNET_*`).
- Never log secrets or sensitive API responses.

## Trailing Stop Integration

- After a successful entry (testnet/mainnet), the executor configures Bybit **native**
  trailing via `POST /v5/position/trading-stop`.
- The strategy also maintains its own progressive trailing state as the source of truth
  for exit decisions; the native trailing is a backup that survives strategy restarts.
- In paper mode, trailing is fully simulated (no exchange call).
