# 08 - Strategy Execution Model

This document captures the current execution model of the `strategy` service after the Tupa `0.8.1`
alignment work.

It is intentionally focused on the current state of the runtime, not the long-term target state.

## Current model

The strategy runtime now operates in three layers:

1. Tupa pipeline contract
   - `config/strategies/viper_smart_copy.tp`
   - defines the validated pipeline shape
   - defines structured step outputs and expected fields
2. Rust-side policy and runtime orchestration
   - `services/strategy/src/main.rs`
   - executes the current trading semantics
   - enriches step results with scores, reasons, and breakdowns
3. Stateful runtime integration
   - open-trade fetch/persistence
   - guard memory
   - trailing runtime state
   - Redis publication
   - PostgreSQL-backed runtime state

## What is already structured

The main policy surfaces now expose structured outputs instead of opaque booleans:

- `validate_entry`
  - `passed`
  - `severity`
  - `reason`
  - `side`
  - `entry_score`
  - `entry_breakdown`
- `check_funding`
  - `passed`
  - `severity`
  - `reason`
  - `funding_rate`
  - `funding_score`
  - `funding_breakdown`
- `calc_smart_size`
  - `quantity`
  - `desired_usdt`
  - `risk_budget_usdt`
  - `volatility_discount`
  - `proposal_score`
  - `reason`
  - `proposal_breakdown`
- `validate_size`
  - `passed`
  - `severity`
  - `reason`
  - `position_usdt`
  - `size_score`
  - `size_breakdown`
- `get_trailing_config`
  - `enabled`
  - trailing parameters
  - `trailing_score`
  - `reason`
- `decision`
  - operational `StrategyDecision` fields
  - `decision_score`
  - `decision_breakdown`
- `audit`
  - `ok`
  - `reason`
  - `decision_action`
  - `decision_score`
  - `smart_copy_compatible`

## What still lives in Rust

The following areas remain intentionally Rust-driven:

- trailing activation, ratcheting, and break-even mechanics
- exit evaluation
- guard state and confirmation memory
- thesis invalidation confirmation state
- trade persistence and open-position lookup
- Redis event publication
- runtime loop and service integration

This is acceptable today because these areas depend on runtime state and side effects.

## Role of the `.tp` pipeline today

The `.tp` file is already part of the real production path, but its role is still narrower than
"full strategy ownership".

Today it primarily provides:

- a validated strategy contract
- a stable execution-plan shape
- structured output expectations for the runtime
- a safer review surface for policy evolution

It does **not** yet contain the full semantics of:

- entry gating
- sizing
- funding validation
- thesis invalidation
- trailing mechanics
- exit policy

Those semantics are still executed in Rust and mirrored into structured outputs.

## Testing gaps

The largest remaining gap in this phase is test depth, not runtime structure.

The strategy service currently has most of its trading semantics in:

- `services/strategy/src/main.rs`

The strategy service now has focused coverage for some of the newer structured outputs directly in
`services/strategy/src/main.rs`, including:

- trailing score and reason summary
- open trade exit trigger selection
- audit summary composition
- structured hold reason reconstruction
- temporal confirmation reason helpers

The remaining gap is breadth, not total absence of tests.

High-value missing tests:

1. `validate_entry`
   - validates `entry_score`
   - validates `entry_breakdown`
   - validates `reason` selection
2. `check_funding`
   - validates `funding_score`
   - validates `funding_breakdown`
3. `calc_smart_size`
   - validates `proposal_score`
   - validates `proposal_breakdown`
4. `validate_size`
   - validates `size_score`
   - validates `size_breakdown`
5. `evaluate_trailing`
   - validates `trailing_score`
   - validates trailing reason composition
6. `evaluate_open_trade_exit`
   - validates trigger selection
   - validates close/hold reasons
7. `decision`
   - validates `decision_score`
   - validates `decision_breakdown`
   - validates propagation of entry/funding/size/trailing reasons
8. `audit`
   - validates summary fields derived from `decision`

## Recommended next step

If this phase is considered feature-complete, the best next investment is:

1. expand the targeted unit-test coverage around the structured outputs
2. keep the public decision event contract stable unless we explicitly decide to publish
   breakdowns externally

That preserves compatibility while increasing confidence in the new strategy model.
