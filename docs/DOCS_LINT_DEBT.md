# Docs Lint Debt (Batch Plan)

## Batch 1 (completed)

- Scope: `README.md`, `docs/*.md`
- Rules temporarily relaxed: MD013, MD022, MD031, MD032, MD040, MD047, MD060
- Outcome: docs lint executable in local CI without blocking delivery.

## Batch 2 (completed)

- Re-enabled: MD022 and MD032.
- Normalized heading/list spacing in:
  - `README.md`
  - `docs/ARCHITECTURE_V2.md`
  - `docs/PHASE1_PLAN.md`
  - `docs/RELEASE_CHECKLIST.md`

## Batch 3

- Re-enable MD031 and MD047.
- Normalize fenced code block spacing and EOF newline style.

## Batch 4

- Scope expansion to `VIPERTRADE_SPEC.md`.
- Re-enable MD040 and MD060 for fenced code language and table formatting.

## Batch 5

- Re-enable MD013 with scoped exceptions only where justified.
- Close this file when all rules are re-enabled.