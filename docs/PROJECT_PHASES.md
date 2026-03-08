# Project Phases - ViperTrade 0.8.x

Derived from `VIPERTRADE_SPEC.md` (blocos 0-15), with formal phase gating for execution.

## Current Snapshot

- Phase 1: Completed (2026-03-08)
- Phase 2: Completed (2026-03-08)
- Phase 3: Completed (2026-03-08)
- Phase 4: Completed (2026-03-08)
- Phase 5: Completed (2026-03-08)
- Phase 6: Completed (2026-03-08)

## Phase 1 - Foundation and Runtime Hardening (Completed)

Scope:

- Architecture baseline, contracts, deterministic runtime path.
- Podman compose hardening and local CI parity.
- Executor live-control, idempotency, fill persistence, reconciliation safety controls.

Spec mapping:

- Blocos 0-7 (core setup through executor)
- Plus initial monitor/reconciliation hardening from Bloco 8

Done evidence:

- End-to-end flow stable on WSL Fedora + Podman.
- CI and CI Local Parity green on closure commit.
- Controlled testnet smoke validated and cleaned.

## Phase 2 - Risk, Reconciliation, and Monitor Maturity (Completed)

Scope:

- Complete monitor-driven risk controls and reconciliation engine.
- Persist reconciliation snapshots and structured drift events.
- Alert routing and operational thresholds.

Spec mapping:

- Bloco 8 (monitor), Bloco 9 (error handling), part of sections 7-9 and 11.

Primary deliverables:

- Persisted reconciliation snapshots with symbol-side drift history.
- Distinct event streams for detection, fix-attempt, fix-result.
- Alert policy matrix (warning/error/critical) wired to Discord.

Done evidence:

- Reconciliation events and snapshots persisted in controlled validation window.
- Alert cooldown and operator playbook documented.
- Validation report: `docs/operations/PHASE2_VALIDATION_2026-03-08.md`.

## Phase 3 - Lead Trader Operations and API Surface (Completed)

Scope:

- Finalize Lead Trader operational controls and public metrics surfaces.
- Harden API endpoints for status, positions, trades, performance, and kill-switch.

Spec mapping:

- Sections 13 and 20, plus copy-trading operational requirements.

Primary deliverables:

- Lead Trader metrics view aligned with public follower expectations.
- Kill-switch, risk status, and copy-health endpoints operational.
- Auth and access patterns for operator-level actions.

Current progress:

- Phase 4 baseline validation script added: `scripts/phase4-validate.sh`.
- Initial baseline evidence: `docs/operations/PHASE4_BASELINE_2026-03-08.md`.

Exit criteria:

- Operator can audit and control runtime via API without direct DB access.
- Public metrics and internal metrics remain consistent over rolling windows.

Done evidence:

- Validation report: `docs/operations/PHASE3_VALIDATION_2026-03-08.md`.
- API `/api/v1` read surface validated (`health`, `status`, `positions`, `trades`, `performance`).
- Kill-switch control validated for both deny-by-default and positive operator flow with audit trail.

## Phase 4 - Backtesting and Paper-to-Live Validation

Scope:

- Consolidate backtesting engine, paper trading loops, and promotion gates.

Spec mapping:

- Section 14 and testing checklists in section 18.

Primary deliverables:

- Reproducible backtest runs with report artifacts.
- Paper trading regression suite against strategy/risk/executor changes.
- Promotion gate: paper -> testnet micro -> controlled live window.

Current progress:

- Phase 4 baseline validation script added: `scripts/phase4-validate.sh`.
- Initial baseline evidence: `docs/operations/PHASE4_BASELINE_2026-03-08.md`.

Exit criteria:

- Backtest + paper reports satisfy risk and drawdown thresholds.
- Promotion decision is evidence-based and documented.

## Phase 5 - Smart Copy and Dynamic Trailing Optimization

Scope:

- Tune Smart Copy behavior and dynamic trailing stop ratcheting for follower compatibility.

Spec mapping:

- Sections 15 and 16, Blocos 14-15.

Primary deliverables:

- Stable position sizing band for Smart Copy reliability.
- Trailing ratchet behavior validated per risk profile.
- Auto-unfollow prevention checks integrated into monitoring.

Current progress:

- Phase 5 plan documented: `docs/PHASE5_SMARTCOPY_TRAILING_PLAN.md`.
- Phase 5 baseline gate added: `scripts/phase5-validate.sh`.
- Initial baseline evidence: `docs/operations/PHASE5_BASELINE_2026-03-08.md`.
- Promotion criteria documented: `docs/operations/PHASE5_PROMOTION_GATE.md`.

Exit criteria:

- Copy success rate and follower-facing metrics hit target ranges.
- Trailing logic preserves gains without excessive churn.

## Phase 6 - Mainnet Micro and Production Readiness

Scope:

- Controlled mainnet micro deployment, DR drills, and release governance.

Spec mapping:

- Bloco 12, sections 8-10, 17-19, and versioning sections 21-22.

Primary deliverables:

- Mainnet micro rollout plan with rollback and disaster recovery drills.
- Secrets rotation and operational security checks automated.
- Release package with changelog, runbook, checklist, and evidence bundle.

Current progress:

- Phase 6 plan documented: `docs/PHASE6_MAINNET_READINESS_PLAN.md`.
- Phase 6 baseline gate added: `scripts/phase6-validate.sh`.
- Initial baseline evidence: `docs/operations/PHASE6_BASELINE_2026-03-08.md`.
- Promotion criteria documented: `docs/operations/PHASE6_PROMOTION_GATE.md`.
- No-mainnet policy documented: `docs/operations/PHASE6_NO_MAINNET_POLICY.md`.
- Testnet micro gate added: `scripts/phase6-testnet-micro-gate.sh`.
- Testnet micro evidence: `docs/operations/PHASE6_TESTNET_MICRO_2026-03-08.md`.
- Decision package closed: `docs/operations/PHASE6_DECISION_PACKAGE_2026-03-08.md`.

Exit criteria:

- Testnet/simulation micro window completes without critical incidents.
- Rollback procedure is tested and time-bounded.
- Project declared production-ready for incremental scaling.

## Governance Rules Across All Phases

- Default-safe runtime posture (`live=false`) outside controlled windows.
- Every phase closes only with objective evidence (logs + DB checks + CI runs).
- No phase promotion with unresolved critical risk or unknown reconciliation drift.
