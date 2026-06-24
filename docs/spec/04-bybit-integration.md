# 04 - Bybit Integration

## Integration Channels

- REST API for order execution, position queries, and account state.
- WebSocket for real-time market data and stream subscriptions.

## Endpoints Configuration

- Public: `wss://stream.bybit.com/v5/public/linear` (mainnet)
- Public: `wss://stream-testnet.bybit.com/v5/public/linear` (testnet)
- Private: `wss://stream.bybit.com/v5/private` (mainnet)
- Private: `wss://stream-testnet.bybit.com/v5/private` (testnet)

## Requirements

- Idempotent order submission via `order_link_id` (UUID-based).
- Retry with exponential backoff for transient failures (max 5 attempts).
- WebSocket reconnection with progressive backoff.
- Request timeout: 5 seconds default, configurable via `BYBIT_RECV_WINDOW`.

## Observability

- Log REST call latency per endpoint.
- Log WebSocket reconnection events and downtime.
- Correlate failures by symbol, order type, and error code.
- Record API error codes 110017, 110094 for troubleshooting.

## Security

- API keys via `.env` (local) or Kubernetes secrets (Kind).
- Never log secrets or API responses containing sensitive data.
- Unified account type (`BYBIT_ACCOUNT_TYPE=UNIFIED`) required.

## Trailing Stop Integration

- After successful entry, executor configures Bybit native trailing via `POST /v5/position/trading-stop`.
- Local trailing state remains as backup when Bybit rejects native trailing setup.
