# 05 - Runtime and Operations

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

- Strategy integration via a versioned `.tp` pipeline.
- The strategy service loads the pipeline in-process through the Tupa parser, typechecker, and codegen layers.
- The `.tp` file currently defines the validated plan shape and structured step contracts used by the runtime.
- Runtime state, exchange data, guard state, and some trading semantics still live in Rust.
- The current migration goal is to move more policy semantics into Tupa-native structured outputs over time.

## Trading Operations and Validation Modes

- Operate as Lead Trader in Bybit Copy Trading Classic.
- Smart Copy optimization with predictable sizing, slippage control, and profile-based leverage limits.
- Self-unfollow protection via reduced failed copies and smaller sizing variance.
- Validation modes before production: stress backtest and paper trading with real data and simulated execution.

## Dynamic Trailing Stop

- Activation by minimum profit and progressive adjustment (ratcheting).
- Trail never loosens; only maintains or tightens.
- Parameters by risk profile to balance protection and trend capture.
- Integration with decision flow and strategy service runtime state.

## Development Blocks

- Blocks 1-15 structure incremental delivery.
- Base project and compose.
- Core services (market-data, strategy, executor, monitor, analytics, ai-analyst).
- Error handling and tests.
- Documentation, micro deploy and Smart Copy/trailing optimizations.
