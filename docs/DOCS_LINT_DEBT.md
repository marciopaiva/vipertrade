# Docs Lint Debt (Batch Plan)

## Batch 1 (completed)

- Scope: `README.md`, `docs/*.md`
- Rules temporarily relaxed to unblock local CI.

## Batch 2 (completed)

- Re-enabled: MD022 and MD032.
- Normalized heading/list spacing in core docs.

## Batch 3 (completed)

- Re-enabled: MD031 and MD047.
- Normalized fenced code block spacing and EOF newline style.

## Batch 4 (completed)

- Scope expanded to `VIPERTRADE_SPEC.md` in local docs lint.
- Re-enabled: MD040 and MD060.

## Batch 5 (completed)

- Re-enabled MD013 globally with practical settings:
  - `line_length=140`
  - ignore code blocks and tables
- No per-file exception currently required after line-length calibration.

## Next

- Gradually reduce the `VIPERTRADE_SPEC.md` MD013 exception by splitting/refactoring sections.
