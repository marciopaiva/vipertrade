# ViperTrade — Cleanup, Business-Rules & Architecture Plan

> Status: living plan. Phase 0 in progress.
> Context: the project grew organically; this plan reassesses which services are
> actually necessary, identifies residue from earlier implementations, and
> revisits the business rules and the proposed microservices architecture.

## 1. What the architecture actually is

Only **3 of 9 services** are on the trading hot path (Redis event bus):

```
market-data ──viper:market_data──▶ strategy ──viper:decisions──▶ executor ──viper:executor_events─▶
                                       ▲
                          analytics (scores via HTTP :8086)
monitor ──viper:reconciliation──▶ (drift / reconciliation, side path)
api + web ──▶ operator layer (HTTP, off the hot path)
ai-analyst ──▶ optional diagnostics (LLM disabled), consumed by the dashboard
backtest ──▶ STUB (health check only, no logic)
```

## 2. Service inventory & verdict

| Service | Lines | Hot path | Evidence | Verdict |
|---|---|---|---|---|
| market-data | 1,664 | yes (publishes `market_data`) | real multi-source consensus | Keep |
| strategy | 5,493 | yes (core) | monolith; real decision logic is **dead code**, pipeline steps are stubs | Keep + refactor (critical) |
| executor | 2,778 | yes (consumes `decisions`) | real paper/live execution | Keep |
| monitor | 732 | side | real reconciliation/drift (SQL-backed) | Keep |
| analytics | 704 | yes (scores for strategy) | real score computation (SQL) | Keep |
| api | 3,034 | operator | recently modularized | Keep |
| web | — | operator | dashboard; weak auth (issue #32) | Keep |
| ai-analyst | 2,567 | optional | LLM disabled; DB-driven diagnostics; not in hot path | Re-evaluate |
| backtest | 71 | no | **pure stub** (health check only) | Remove (Phase 0) |

**Takeaway:** there are not "many useless services" — 6 of 9 are real and integrated.
The questionable ones are `backtest` (empty stub) and `ai-analyst` (2.5k lines with
LLM disabled, off the hot path). The real growth problem is the `strategy` monolith
with disconnected logic.

## 3. Residue inventory

| Residue | What | Action |
|---|---|---|
| `config/strategies/viper_smart_copy.tp` | 485 lines, orphaned (runtime load already removed) | Relocate to `docs/spec/` as business-rules reference |
| `docs/legacy/` | old `VIPERTRADE_SPEC.md` + README | Remove |
| `ANALYSIS_DEEP_DIVE.md` (40 KB, root) | likely stale analysis | Evaluate (kept for now) |
| dead decision logic in `strategy` | `execute_strategy_step` produces ENTER_LONG/SHORT but is bypassed by stub steps | Business decision (issue #33) |
| `backtest` stub | empty service | Remove from workspace + k8s + compose + scripts |

Historical operational evidence (`docs/operations/evidence/PHASE4_BACKTEST_*`) is a
record and is intentionally retained.

## 4. Business-rules reassessment

The real rules (entry score, sizing, funding carry, trailing, cooldown, thesis) exist
in **dead code** (`execute_strategy_step` → ENTER_LONG/SHORT) and in the `.tp` file
(the former spec). The `ViperSmartCopy` pipeline steps are stubs (`passed:true` / HOLD),
so no trades open.

Decision required (Phase 1): (a) revive the real logic into the `pipeline!` steps, or
(b) rewrite the rules from scratch using the dead code / `.tp` as the reference spec.
Once rules are real, gate trades on tupa constraint failures (`result.failures`) — the
runtime currently ignores them; the `equity_floor` constraint is the first hook.

## 5. Architecture reassessment

Central question: do 9 microservices + Kind/K8s justify themselves for a single-strategy
paper bot? Operational overhead is high (9 Dockerfiles, 9 deployments, registry, compose)
with no current scale/fault-isolation requirement.

- **Option A (consolidate, recommended for now):** collapse into ~3–4 binaries — a `core`
  (market-data + strategy + executor pipeline), an `ops-api` (api + analytics), `web`, with
  monitor/ai-analyst as opt-in. Drastically reduces Docker/K8s surface for the same capability.
- **Option B (keep microservices):** justifiable only with a real scale/isolation plan.

## 6. Phased execution

- **Phase 0 — low-risk quick wins (this PR):** remove `backtest` stub (code + k8s + compose +
  scripts), remove `docs/legacy/`, relocate the orphaned `.tp`, audit unused deps.
- **Phase 1 — business rules:** decide revive vs rewrite (#33); document the live rules spec.
- **Phase 2 — architecture consolidation (if Option A):** merge hot-path services / reduce
  deployments; re-evaluate ai-analyst.
- **Phase 3 — security & enforcement:** fix auth (#32); gate trades on tupa constraints.

## 7. Risks

- Consolidating services changes the deploy model — do it behind Kind validation.
- Reviving dead decision logic changes trading behavior — validate exhaustively in paper.
- `ai-analyst`/`analytics` have consumers (dashboard, scores) — verify before removing.
